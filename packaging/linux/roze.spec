Name:           roze
Version:        2.0.0
Release:        1%{?dist}
Summary:        The Roze Programming Language Toolchain (v2.0.0 'Prometheus')

License:        MIT
URL:            https://roze-lang.org
Source0:        roze-%{version}.tar.gz

Requires:       gcc
Suggests:       java-17-openjdk-devel

%description
Roze is an expressive, autonomous, fast, statically typed systems programming
language supporting dual native Cranelift/C code generation and JVM
execution, deterministic automatic reference counting, safe concurrency,
bare-metal kernels, microcontroller IoT, and WebAssembly targets.

%prep
%setup -q

%build
# Precompiled in release pipeline

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_prefix}/lib/roze/stdlib

for bin in roze bsr roze-build roze-pkg roze-lsp roze-bindgen roze-svd rozedoc; do
    if [ -f target/release/${bin} ]; then
        install -m 755 target/release/${bin} %{buildroot}%{_bindir}/${bin}
    elif [ -f target/debug/${bin} ]; then
        install -m 755 target/debug/${bin} %{buildroot}%{_bindir}/${bin}
    fi
done

if [ -d stdlib/src ]; then
    cp -rf stdlib/src/* %{buildroot}%{_prefix}/lib/roze/stdlib/
elif [ -d stdlib ]; then
    cp -rf stdlib/* %{buildroot}%{_prefix}/lib/roze/stdlib/
fi

%files
%{_bindir}/roze
%{_bindir}/bsr
%{_bindir}/roze-build
%{_bindir}/roze-pkg
%{_bindir}/roze-lsp
%{_bindir}/roze-bindgen
%{_bindir}/roze-svd
%{_bindir}/rozedoc
%{_prefix}/lib/roze/stdlib/*
%doc README.md
%doc docs/GRAMMAR.md
%doc docs/STDLIB.md
%license LICENSE

%changelog
* Sun Nov 15 2026 Firoze <firoze@roze-lang.org> - 2.0.0-1
- Official v2.0.0 'Prometheus' production release.
- ISO/IEC 14977 formal grammar specification.
- Full 35-module standard library.
- Zero-diff 3-stage bootstrap fixpoint closure.
- Unified bsr/roze CLI driver.
- Microcontroller IoT & PAC SVD toolchain.
