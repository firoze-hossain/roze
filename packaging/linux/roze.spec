Name:           roze
Version:        1.0.0
Release:        1%{?dist}
Summary:        The Roze Programming Language Toolchain

License:        MIT
URL:            https://roze-lang.org
Source0:        roze-%{version}.tar.gz

Requires:       gcc
Suggests:       java-17-openjdk-devel

%description
Roze is an expressive, fast, statically typed systems programming
language supporting dual native Cranelift/C code generation and JVM
execution, automatic reference counting, safe concurrency, and bare-metal
freestanding kernel capabilities.

%prep
%setup -q

%build
# Precompiled in release pipeline

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_prefix}/lib/roze/stdlib

install -m 755 target/release/roze %{buildroot}%{_bindir}/roze
install -m 755 target/release/roze-build %{buildroot}%{_bindir}/roze-build
install -m 755 target/release/roze-pkg %{buildroot}%{_bindir}/roze-pkg
install -m 755 target/release/roze-lsp %{buildroot}%{_bindir}/roze-lsp
cp -rf stdlib/* %{buildroot}%{_prefix}/lib/roze/stdlib/

%files
%{_bindir}/roze
%{_bindir}/roze-build
%{_bindir}/roze-pkg
%{_bindir}/roze-lsp
%{_prefix}/lib/roze/stdlib/*
%doc README.md
%license LICENSE

%changelog
* Mon Oct 27 2026 Firoze <firoze@roze-lang.org> - 1.0.0-1
- Official v1.0.0 stable production release.
