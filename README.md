# bpfquery

An experiment with compiling SQL to BPF programs. Only the following example works and the backend target is only bpftrace running on the targeted server.

```bash
git clone git@github.com:zmaril/bpfquery.git
cd bpfquery
cargo install --path .
bpfquery devserver #some linux server you have ssh access to that has bpftrace installed on it 
> select pid, cpu, elapsed from kprobe.do_nanosleep;
# watch as bpftrace sends info back about things
```

# Motivation

It took me ten years and two weeks to write my first BPF program from scratch that actually worked. I learned about eBPF back in the early 2010's and was too intimidated and flummoxed by the tooling to do anything of consequence of it. I loved the idea of getting information out of the kernel and seeing what was _really_ going on in there, however, I was never able to get past "hello world". Other projects and ideas where easier and more fruitful to do, so I did those the last decade. [Finding myself with some time during a job transition](https://www.linkedin.com/in/zack-maril/), I told myself I would finally write a BPF program after many years of putting it off and just using bpftrace. And I did! It sucked! It took two weeks of really frustrating work to just get "hello world" to run in a container on my laptop. It was not fun! 

During this time, I read a lot of programs that others had written and saw the tools that were referenced and used most often that showed the promise of BPF. This reinforced two things for me that I had felt for a long time:

1. Writing bpf programs is hard if you don't have a lot of experience with operating system internals and compilers.
2. Most bpf programs are continuous queries on streams of events coming from the kernel.

Confirmation bias aside, I thought that if I could write a SQL parser and compiler that could take a SQL query and turn it into a BPF program, I could make it easier for people (i.e. me) to write use BPF-based programs and better understand what's going on in the kernel. I had bought and read through [a book on complex event processing](https://www.amazon.com/Power-Events-Introduction-Processing-Distributed/dp/0201727897) many years ago and recently saw [ksqlDB](https://ksqldb.io/) used to great effect at my previous job, so I figured why not take a stab at it while I have some time. So far, it's been a lot of fun and I've learned a lot about SQL and BPF in the process. Maybe someday, others will find it useful too.

# Design/Roadmap 

Right now bpfquery is (sort of) working. It's got a repl that (sort of works), a sql to bpftrace compiler that (sort of) works, and an executor that (sort of) works. What's next up is removing those (sort of) qualifiers and making it just work. 

The main focus right now is expanding the SQL to BPF compiler to handle more SQL queries, with incidental improvements to the repl and executor as needed. Joins don't work, only the builtin bpftrace arguments like pid and comm are supported, expressions are not supported, and there's no streaming semantics yet. So figuring out how to make those work is the next step. Much later on, after I've nailed down the semantics of the language, I'd like to make it so that the backend can be switched out for other BPF backends like libbpf or bcc. But there's a lot of experimentation to do before that happens.