# bpfquery

An experiment with compiling SQL to bpf programs. Currently only the following example really works and the target is bpftrace only. Eventually, maybe, full SQL semantics with stream processing will be implemented and actually bpf programs will be emitted, not just bpftrace scripts. Next up is joins, struct access syntax, expr compiling and streaming semantics. 

```bash
git clone git@github.com:zmaril/bpfquery.git
cd bpfquery
cargo run devserver #some linux server you have ssh access to
> select pid, cpu, elapsed from kprobe.do_nanosleep;
# watch as bpftrace sends info back about things
```
