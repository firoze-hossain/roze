// compiler/src/codegen/native.rs
//
// A native (non-JVM) backend, built on Cranelift, targeting the host
// machine directly -- no JVM/JDK involved at any point. This exists to
// prove the typed IR (see ir.rs) is genuinely backend-agnostic, and as
// the starting point for the systems/embedded half of the project's
// goals that the JVM backend structurally can't reach (see
// ROADMAP.md's "The bigger picture" section).
//
// STATUS: an intentionally minimal SPIKE, not a production backend --
// exactly what the roadmap calls for at this stage ("an LLVM/Cranelift
// spike"), not a finished systems-programming story. Deliberately out
// of scope for now, because it depends on the not-yet-decided memory
// model (see docs/MEMORY_MODEL_DECISION.md):
//   - Any heap-allocated value: general strings (beyond literals),
//     `list`, `map`. There's no allocator, no GC, no ARC wired up yet --
//     that's precisely the decision this spike is deferring to, not
//     working around.
//   - Every Core/Collections/IO/Web/Database intrinsic. They're JVM
//     standard-library calls today; a native equivalent needs the
//     memory model decided first (e.g. what does a native `list` even
//     own its elements *as*?).
//
// What IS supported, for real: functions with `int`/`bool` parameters
// and return types, arithmetic, comparisons, boolean logic (with real
// short-circuit evaluation), `if`/`else`/`while`/`for`, calling other
// Roze functions, and `println` of an int/bool/string-literal. That's
// enough to prove the pipeline end-to-end -- typed IR in, a real,
// runnable, non-JVM native executable out -- without first having to
// resolve the bigger decision that heap-allocated types need.
use crate::ir::{TypedExpression, TypedExpressionKind, TypedProgram, TypedStatement};
use crate::parser::ast::{BinaryOperator, UnaryOperator};
use crate::semantic::Type;
use anyhow::{anyhow, Result};
use cranelift::prelude::*;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

/// Every Roze value on the native backend is represented as a 64-bit
/// integer for this spike -- including booleans, as 0/1. A real
/// implementation would want a narrower type for `bool` and a real
/// pointer-sized representation once heap types exist; using one
/// uniform type everywhere keeps this spike's code generation simple
/// while it's only proving the pipeline works at all.
const VALUE_TYPE: types::Type = types::I64;

/// Compiles `program` (the same typed IR the JVM backend consumes) to a
/// native object file, then links it into an executable using the
/// system's C compiler -- present on essentially every dev machine
/// already, and already the same kind of reliance the JVM backend has
/// on an external tool (`javac`).
pub fn compile_to_native(program: TypedProgram, input_file: &str) -> Result<()> {
    let output_name = crate::codegen::class_name_from_path(input_file);

    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").map_err(|e| anyhow!(e.to_string()))?;
    flag_builder.set("is_pic", "true").map_err(|e| anyhow!(e.to_string()))?;
    let isa_builder = cranelift_native::builder()
        .map_err(|e| anyhow!("unsupported host platform for the native backend: {}", e))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;

    let obj_builder = ObjectBuilder::new(isa, output_name.clone(), cranelift_module::default_libcall_names())
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut module = ObjectModule::new(obj_builder);

    {
        let mut generator = NativeGenerator::new(&mut module)?;
        generator.declare_all_functions(&program)?;
        generator.compile_all_functions(&program)?;
    }

    let product = module.finish();
    let bytes = product.emit().map_err(|e| anyhow!(e.to_string()))?;
    let obj_path = format!("{}.o", output_name);
    fs::write(&obj_path, &bytes)?;
    println!("📝 Generated native object file: {}", obj_path);

    let status = Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(&output_name)
        .status()
        .map_err(|e| anyhow!("failed to invoke the system linker ('cc'): {}", e))?;

    if !status.success() {
        return Err(anyhow!("Linking failed"));
    }
    println!("✅ Linked native executable: {}", output_name);

    Ok(())
}

/// Runs a compiled native executable by path (as opposed to
/// `codegen::run_java`, which runs a JVM class by name through `java`).
pub fn run_native(name: &str) -> Result<()> {
    let exe_path = if name.starts_with('.') || name.contains('/') || name.contains('\\') {
        name.to_string()
    } else {
        format!("./{}", name)
    };
    let status = Command::new(&exe_path)
        .status()
        .map_err(|e| anyhow!("failed to run '{}': {}", exe_path, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("native program exited with a non-zero status"))
    }
}

/// Rejects any type this backend doesn't support yet (see the module
/// doc comment for why), with a message pointing at why, rather than
/// either panicking or silently miscompiling.
fn require_supported_type(ty: &Type, context: &str) -> Result<()> {
    match ty {
        Type::Int | Type::Bool | Type::Void | Type::Unknown => Ok(()),
        Type::String => Err(anyhow!(
            "the native backend doesn't support string values yet (only a string literal passed directly to println) -- {}",
            context
        )),
        Type::List | Type::Map => Err(anyhow!(
            "the native backend doesn't support '{}' yet -- it's heap-allocated, and the native backend has no memory model decided yet (see docs/MEMORY_MODEL_DECISION.md) -- {}",
            ty, context
        )),
        Type::Function { .. } => Err(anyhow!("the native backend doesn't support function values -- {}", context)),
    }
}

struct NativeGenerator<'a> {
    module: &'a mut ObjectModule,
    functions: HashMap<String, FuncId>,
    printf_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    literal_counter: usize,
}

impl<'a> NativeGenerator<'a> {
    fn new(module: &'a mut ObjectModule) -> Result<Self> {
        // printf's Cranelift-side signature only needs to reflect what
        // every call site here actually passes: a format-string pointer
        // plus exactly one more 64-bit value (unused by the callee when
        // the format string has no '%' specifier -- harmless, and it
        // means every call site can share one signature instead of
        // needing a distinct one per arg count for a variadic function).
        let mut printf_sig = module.make_signature();
        printf_sig.params.push(AbiParam::new(VALUE_TYPE));
        printf_sig.params.push(AbiParam::new(VALUE_TYPE));
        printf_sig.returns.push(AbiParam::new(types::I32));
        let printf_id = module.declare_function("printf", Linkage::Import, &printf_sig)
            .map_err(|e| anyhow!(e.to_string()))?;

        let fmt_int = Self::declare_c_string(module, "__roze_fmt_int", b"%lld\n")?;
        let fmt_str = Self::declare_c_string(module, "__roze_fmt_str", b"%s\n")?;
        let true_str = Self::declare_c_string(module, "__roze_true_str", b"true")?;
        let false_str = Self::declare_c_string(module, "__roze_false_str", b"false")?;

        Ok(Self {
            module,
            functions: HashMap::new(),
            printf_id,
            fmt_int,
            fmt_str,
            true_str,
            false_str,
            literal_counter: 0,
        })
    }

    fn declare_c_string(module: &mut ObjectModule, name: &str, bytes: &[u8]) -> Result<DataId> {
        let mut owned = bytes.to_vec();
        owned.push(0); // NUL-terminate: this is handed straight to printf as a C string.
        let mut data_ctx = DataDescription::new();
        data_ctx.define(owned.into_boxed_slice());
        let data_id = module.declare_data(name, Linkage::Local, false, false)
            .map_err(|e| anyhow!(e.to_string()))?;
        module.define_data(data_id, &data_ctx).map_err(|e| anyhow!(e.to_string()))?;
        Ok(data_id)
    }

    /// Registers every top-level function's signature before compiling
    /// any bodies, so forward references and mutual recursion work (the
    /// same reason the type checker and the JVM backend each do their
    /// own first pass). Roze's `main` is renamed to `__roze_main` here
    /// so it doesn't collide with the real C-ABI `main` synthesized in
    /// `emit_c_main`.
    fn declare_all_functions(&mut self, program: &TypedProgram) -> Result<()> {
        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, return_type, .. } = stmt {
                for param in params {
                    require_supported_type(&param.type_, &format!("parameter '{}' of function '{}'", param.name, name))?;
                }
                require_supported_type(return_type, &format!("the return type of function '{}'", name))?;

                let mut sig = self.module.make_signature();
                for _ in params {
                    sig.params.push(AbiParam::new(VALUE_TYPE));
                }
                if !matches!(return_type, Type::Void) {
                    sig.returns.push(AbiParam::new(VALUE_TYPE));
                }

                let func_name = if name == "main" { "__roze_main" } else { name.as_str() };
                let func_id = self.module.declare_function(func_name, Linkage::Local, &sig)
                    .map_err(|e| anyhow!(e.to_string()))?;
                self.functions.insert(name.clone(), func_id);
            }
        }
        Ok(())
    }

    fn compile_all_functions(&mut self, program: &TypedProgram) -> Result<()> {
        let mut ctx = self.module.make_context();

        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, body, .. } = stmt {
                let func_id = *self.functions.get(name)
                    .expect("declare_all_functions registers every function before this runs");

                ctx.func.signature = self.module.declarations().get_function_decl(func_id).signature.clone();

                {
                    let mut func_ctx = FunctionBuilderContext::new();
                    let builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

                    let mut fc = FunctionCompiler {
                        module: self.module,
                        builder,
                        functions: &self.functions,
                        printf_id: self.printf_id,
                        fmt_int: self.fmt_int,
                        fmt_str: self.fmt_str,
                        true_str: self.true_str,
                        false_str: self.false_str,
                        literal_counter: &mut self.literal_counter,
                        scopes: vec![HashMap::new()],
                        next_var_index: 0,
                    };

                    let entry = fc.builder.create_block();
                    fc.builder.append_block_params_for_function_params(entry);
                    fc.builder.switch_to_block(entry);
                    fc.builder.seal_block(entry);

                    for (i, param) in params.iter().enumerate() {
                        let var = fc.declare_local(&param.name);
                        let value = fc.builder.block_params(entry)[i];
                        fc.builder.def_var(var, value);
                    }

                    let terminated = fc.compile_statement(body)?;
                    if !terminated {
                        fc.builder.ins().return_(&[]);
                    }
                    fc.builder.finalize();
                }

                self.module.define_function(func_id, &mut ctx).map_err(|e| anyhow!(e.to_string()))?;
                self.module.clear_context(&mut ctx);
            }
        }

        self.emit_c_main(&mut ctx)?;
        Ok(())
    }

    /// Emits the real C-ABI `main() -> i32` entry point, which just
    /// calls Roze's `main` (compiled above as `__roze_main`) and returns
    /// 0 -- the same reason the JVM backend hard-codes `main`'s Java
    /// signature regardless of what Roze's own `main` declares.
    fn emit_c_main(&mut self, ctx: &mut cranelift::codegen::Context) -> Result<()> {
        let mut main_sig = self.module.make_signature();
        main_sig.returns.push(AbiParam::new(types::I32));
        let main_id = self.module.declare_function("main", Linkage::Export, &main_sig)
            .map_err(|e| anyhow!(e.to_string()))?;

        ctx.func.signature = main_sig;
        {
            let mut func_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let block = builder.create_block();
            builder.switch_to_block(block);
            builder.seal_block(block);

            if let Some(&roze_main_id) = self.functions.get("main") {
                let main_ref = self.module.declare_func_in_func(roze_main_id, builder.func);
                builder.ins().call(main_ref, &[]);
            }
            // Else: no `main` in the Roze program -- still emit a valid
            // (trivial) executable rather than failing to link, matching
            // the JVM backend's "No main function found!" fallback.

            let zero = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[zero]);
            builder.finalize();
        }
        self.module.define_function(main_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
        self.module.clear_context(ctx);
        Ok(())
    }
}

/// Compiles the body of a single function. Holds a live `FunctionBuilder`
/// for the whole duration, plus everything it needs to emit calls to
/// other Roze functions or the printf-based intrinsics -- all declared
/// once on `NativeGenerator` and threaded through here by reference.
struct FunctionCompiler<'a, 'b> {
    module: &'a mut ObjectModule,
    builder: FunctionBuilder<'b>,
    functions: &'a HashMap<String, FuncId>,
    printf_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    /// Shared across every function being compiled (not reset per
    /// function), so two string literals never collide on the same
    /// generated data symbol name.
    literal_counter: &'a mut usize,
    scopes: Vec<HashMap<String, Variable>>,
    next_var_index: usize,
}

impl<'a, 'b> FunctionCompiler<'a, 'b> {
    fn declare_local(&mut self, name: &str) -> Variable {
        let var = Variable::new(self.next_var_index);
        self.next_var_index += 1;
        self.builder.declare_var(var, VALUE_TYPE);
        self.scopes.last_mut().expect("at least one scope").insert(name.to_string(), var);
        var
    }

    fn lookup_local(&self, name: &str) -> Variable {
        for scope in self.scopes.iter().rev() {
            if let Some(&var) = scope.get(name) {
                return var;
            }
        }
        panic!(
            "undefined variable '{}' reached native codegen -- the type checker should have rejected this before codegen ever ran",
            name
        );
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Compiles a statement. Returns whether control flow is guaranteed
    /// to have already left the function by the time this statement
    /// finishes (e.g. because it always executes a `return`) -- callers
    /// use this to avoid emitting an unreachable extra jump/return after
    /// a block that already terminated, since Cranelift requires every
    /// block to end in exactly one terminator instruction.
    fn compile_statement(&mut self, stmt: &TypedStatement) -> Result<bool> {
        match stmt {
            TypedStatement::Block { statements, .. } => {
                self.push_scope();
                let mut terminated = false;
                for s in statements {
                    if terminated {
                        break; // dead code after an unconditional return
                    }
                    terminated = self.compile_statement(s)?;
                }
                self.pop_scope();
                Ok(terminated)
            }
            TypedStatement::Let { name, value, .. } => {
                require_supported_type(&value.type_, &format!("variable '{}'", name))?;
                let val = self.compile_expression(value)?;
                let var = self.declare_local(name);
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Assign { name, value, .. } => {
                let val = self.compile_expression(value)?;
                let var = self.lookup_local(name);
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Expression { expr, .. } => {
                self.compile_expression(expr)?;
                Ok(false)
            }
            TypedStatement::Return { value, .. } => {
                match value {
                    Some(expr) => {
                        let val = self.compile_expression(expr)?;
                        self.builder.ins().return_(&[val]);
                    }
                    None => {
                        self.builder.ins().return_(&[]);
                    }
                }
                Ok(true)
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                let cond_val = self.compile_expression(condition)?;

                let then_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                let else_block = if else_branch.is_some() { self.builder.create_block() } else { merge_block };

                self.builder.ins().brif(cond_val, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                let then_terminated = self.compile_statement(then_branch)?;
                if !then_terminated {
                    self.builder.ins().jump(merge_block, &[]);
                }

                let else_terminated = if let Some(else_stmt) = else_branch {
                    self.builder.switch_to_block(else_block);
                    self.builder.seal_block(else_block);
                    let terminated = self.compile_statement(else_stmt)?;
                    if !terminated {
                        self.builder.ins().jump(merge_block, &[]);
                    }
                    terminated
                } else {
                    false
                };

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                Ok(else_branch.is_some() && then_terminated && else_terminated)
            }
            TypedStatement::While { condition, body, .. } => {
                let header = self.builder.create_block();
                let loop_body = self.builder.create_block();
                let exit = self.builder.create_block();

                self.builder.ins().jump(header, &[]);

                self.builder.switch_to_block(header);
                let cond_val = self.compile_expression(condition)?;
                self.builder.ins().brif(cond_val, loop_body, &[], exit, &[]);

                self.builder.switch_to_block(loop_body);
                self.builder.seal_block(loop_body);
                let body_terminated = self.compile_statement(body)?;
                if !body_terminated {
                    self.builder.ins().jump(header, &[]);
                }
                self.builder.seal_block(header);

                self.builder.switch_to_block(exit);
                self.builder.seal_block(exit);
                Ok(false)
            }
            TypedStatement::For { init, condition, update, body, .. } => {
                self.push_scope();
                self.compile_statement(init)?;

                let header = self.builder.create_block();
                let loop_body = self.builder.create_block();
                let exit = self.builder.create_block();

                self.builder.ins().jump(header, &[]);

                self.builder.switch_to_block(header);
                let cond_val = self.compile_expression(condition)?;
                self.builder.ins().brif(cond_val, loop_body, &[], exit, &[]);

                self.builder.switch_to_block(loop_body);
                self.builder.seal_block(loop_body);
                let body_terminated = self.compile_statement(body)?;
                if !body_terminated {
                    self.compile_statement(update)?;
                    self.builder.ins().jump(header, &[]);
                }
                self.builder.seal_block(header);

                self.builder.switch_to_block(exit);
                self.builder.seal_block(exit);
                self.pop_scope();
                Ok(false)
            }
            TypedStatement::Function { .. } => {
                // Nested functions aren't supported; handled at the top level.
                Ok(false)
            }
        }
    }

    fn compile_expression(&mut self, expr: &TypedExpression) -> Result<Value> {
        match &expr.kind {
            TypedExpressionKind::Number(s) => {
                let n: i64 = s.parse().map_err(|_| anyhow!("invalid integer literal '{}'", s))?;
                Ok(self.builder.ins().iconst(VALUE_TYPE, n))
            }
            TypedExpressionKind::Boolean(b) => {
                Ok(self.builder.ins().iconst(VALUE_TYPE, if *b { 1 } else { 0 }))
            }
            TypedExpressionKind::Null => Ok(self.builder.ins().iconst(VALUE_TYPE, 0)),
            TypedExpressionKind::String(_) => Err(anyhow!(
                "the native backend only supports a string literal passed directly to println(), not as a general value"
            )),
            TypedExpressionKind::Identifier(name) => {
                let var = self.lookup_local(name);
                Ok(self.builder.use_var(var))
            }
            TypedExpressionKind::Unary { operator, operand } => {
                let val = self.compile_expression(operand)?;
                match operator {
                    UnaryOperator::Negate => Ok(self.builder.ins().ineg(val)),
                    UnaryOperator::Not => {
                        let zero = self.builder.ins().iconst(VALUE_TYPE, 0);
                        Ok(self.compile_cmp(IntCC::Equal, val, zero))
                    }
                }
            }
            TypedExpressionKind::Binary { left, operator, right } => self.compile_binary(left, operator, right),
            TypedExpressionKind::Call { function, arguments } => self.compile_call(function, arguments),
        }
    }

    fn compile_cmp(&mut self, cc: IntCC, l: Value, r: Value) -> Value {
        let cmp = self.builder.ins().icmp(cc, l, r);
        self.builder.ins().uextend(VALUE_TYPE, cmp)
    }

    fn compile_binary(&mut self, left: &TypedExpression, operator: &BinaryOperator, right: &TypedExpression) -> Result<Value> {
        // And/Or need real short-circuit control flow (the right-hand
        // side must not be evaluated when the left already decides the
        // result), not just a plain bitwise instruction.
        match operator {
            BinaryOperator::And => return self.compile_short_circuit(left, right, true),
            BinaryOperator::Or => return self.compile_short_circuit(left, right, false),
            _ => {}
        }

        let l = self.compile_expression(left)?;
        let r = self.compile_expression(right)?;

        Ok(match operator {
            BinaryOperator::Add => self.builder.ins().iadd(l, r),
            BinaryOperator::Subtract => self.builder.ins().isub(l, r),
            BinaryOperator::Multiply => self.builder.ins().imul(l, r),
            BinaryOperator::Divide => self.builder.ins().sdiv(l, r),
            BinaryOperator::Equal => self.compile_cmp(IntCC::Equal, l, r),
            BinaryOperator::NotEqual => self.compile_cmp(IntCC::NotEqual, l, r),
            BinaryOperator::LessThan => self.compile_cmp(IntCC::SignedLessThan, l, r),
            BinaryOperator::GreaterThan => self.compile_cmp(IntCC::SignedGreaterThan, l, r),
            BinaryOperator::LessEqual => self.compile_cmp(IntCC::SignedLessThanOrEqual, l, r),
            BinaryOperator::GreaterEqual => self.compile_cmp(IntCC::SignedGreaterThanOrEqual, l, r),
            BinaryOperator::And | BinaryOperator::Or => unreachable!("handled above"),
        })
    }

    fn compile_short_circuit(&mut self, left: &TypedExpression, right: &TypedExpression, is_and: bool) -> Result<Value> {
        let l = self.compile_expression(left)?;

        let rhs_block = self.builder.create_block();
        let skip_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, VALUE_TYPE);

        if is_and {
            self.builder.ins().brif(l, rhs_block, &[], skip_block, &[]);
        } else {
            self.builder.ins().brif(l, skip_block, &[], rhs_block, &[]);
        }

        self.builder.switch_to_block(rhs_block);
        self.builder.seal_block(rhs_block);
        let r = self.compile_expression(right)?;
        self.builder.ins().jump(merge_block, &[r]);

        self.builder.switch_to_block(skip_block);
        self.builder.seal_block(skip_block);
        let short_circuit_val = self.builder.ins().iconst(VALUE_TYPE, if is_and { 0 } else { 1 });
        self.builder.ins().jump(merge_block, &[short_circuit_val]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(self.builder.block_params(merge_block)[0])
    }

    fn compile_call(&mut self, name: &str, arguments: &[TypedExpression]) -> Result<Value> {
        if name == "println" {
            return self.compile_println(arguments);
        }

        if super::jvm::is_intrinsic(name) {
            return Err(anyhow!(
                "'{}' is a Core/Collections/IO/Web/Database intrinsic, which is only available on the JVM backend today (see docs/MEMORY_MODEL_DECISION.md for why the native backend doesn't have these yet)",
                name
            ));
        }

        let func_id = *self.functions.get(name).ok_or_else(|| {
            anyhow!(
                "undefined function '{}' reached native codegen -- the type checker should have rejected this before codegen ever ran",
                name
            )
        })?;
        let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);

        let mut arg_values = Vec::with_capacity(arguments.len());
        for arg in arguments {
            arg_values.push(self.compile_expression(arg)?);
        }

        let call = self.builder.ins().call(func_ref, &arg_values);
        let results = self.builder.inst_results(call);
        if results.is_empty() {
            // A void-returning function used where a value is
            // syntactically expected (only reachable as a bare
            // Expression-statement in practice, since the type checker
            // wouldn't allow using a void value otherwise) -- return an
            // unused placeholder so callers don't need special-casing.
            Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
        } else {
            Ok(results[0])
        }
    }

    fn compile_println(&mut self, arguments: &[TypedExpression]) -> Result<Value> {
        if arguments.len() != 1 {
            return Err(anyhow!("the native backend's println only supports exactly one argument for now"));
        }
        let arg = &arguments[0];

        if let TypedExpressionKind::String(s) = &arg.kind {
            // A literal is compile-time-constant data -- printable
            // directly with no general string/heap type needed.
            *self.literal_counter += 1;
            let name = format!("__roze_str_lit_{}", self.literal_counter);
            let data_id = NativeGenerator::declare_c_string(self.module, &name, s.as_bytes())?;
            let gv = self.module.declare_data_in_func(data_id, self.builder.func);
            let str_ptr = self.builder.ins().global_value(VALUE_TYPE, gv);
            return self.emit_printf(self.fmt_str, str_ptr);
        }

        match &arg.type_ {
            Type::Bool => {
                let val = self.compile_expression(arg)?;
                let true_gv = self.module.declare_data_in_func(self.true_str, self.builder.func);
                let false_gv = self.module.declare_data_in_func(self.false_str, self.builder.func);
                let true_ptr = self.builder.ins().global_value(VALUE_TYPE, true_gv);
                let false_ptr = self.builder.ins().global_value(VALUE_TYPE, false_gv);
                let selected_ptr = self.builder.ins().select(val, true_ptr, false_ptr);
                self.emit_printf(self.fmt_str, selected_ptr)
            }
            Type::Int | Type::Unknown => {
                let val = self.compile_expression(arg)?;
                self.emit_printf(self.fmt_int, val)
            }
            Type::String => Err(anyhow!(
                "the native backend only supports println of a string *literal* (e.g. println(\"hi\")), not a general string value -- see docs/MEMORY_MODEL_DECISION.md"
            )),
            other => Err(anyhow!("println of a {} isn't supported on the native backend yet", other)),
        }
    }

    fn emit_printf(&mut self, fmt_data: DataId, arg: Value) -> Result<Value> {
        let fmt_gv = self.module.declare_data_in_func(fmt_data, self.builder.func);
        let fmt_ptr = self.builder.ins().global_value(VALUE_TYPE, fmt_gv);
        let printf_ref = self.module.declare_func_in_func(self.printf_id, self.builder.func);
        let call = self.builder.ins().call(printf_ref, &[fmt_ptr, arg]);
        let _ = self.builder.inst_results(call);
        Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
    }
}
