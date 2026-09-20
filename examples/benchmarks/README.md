# Benchmarks: hello world vs C / C++

Equivalent programs printing `hello` (no newline), compiled with:

```bash
gcc -O2 hello.c -o hello_c
g++ -O2 hello.cpp -o hello_cpp
rubyc build hello.rc --target linux_x86_64 -o hello_rc
```

Runtimes measured on the x86_64 dev VM (200-run average, includes process
spawn). Re-run anytime with the same commands; binaries must execute on the
VM (`ssh rubyc-dev`), never on the host laptop.
