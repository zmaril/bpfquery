# bpfquery

An experiment with compiling SQL to BPF(trace) programs. 

```bash
git clone git@github.com:zmaril/bpfquery.git
cd bpfquery
cargo run devserver #some linux server you have ssh access to that has bpftrace installed on it 
# open up localhost:3030
```
<a href="https://asciinema.org/a/672845" target="_blank"><img src="https://asciinema.org/a/672845.svg" /></a>

# Queries that work right now 
```sql
select pid, cpu, elapsed from kprobe.do_nanosleep; // getting some basic info from a kprobe
select str(args.filename) from tracepoint.syscalls.sys_enter_openat; //get the filename from a tracepoint
select * from kprobe.do_nanosleep where pid > 1000; // filters 
```

# Queries that don't work right now 
```sql
//stdin:1:26-27: WARNING: comparison of integers of different signs: 'unsigned int64' and 'int64' can lead to undefined behavior
select * from kprobe.do_nanosleep where pid > 2*1000
//Working on tumble and streaming semantics
SELECT tumble(interval '10 seconds') as bucket, count(*) FROM kprobe.do_nanosleep GROUP BY bucket;
```

# Related Work

* [bpftrace](https://github.com/bpftrace/bpftrace)
* ebql - [code](https://github.com/ringtack/ebql) and [paper](https://etos.cs.brown.edu/publications/theses/rtang-honors.pdf)
* [Arroyo](https://arroyo.dev/) - a stream processing engine that bpfquery is inspiring a lot of bpfquery design so far.

# Progress so far 

* [x] Expressions - a lot of expressions just work so far, but there's a lot of edge cases to handle to as they come up, but the expectation is that something like `select pid + 1 from kprobe.do_nanosleep` should work.
* [x] Predicates/filtering/`where` - `where` clauses get parsed and compiled into predicates and often work. `select * from kprobe.do_nanosleep where pid > 1000` should work fine.
* [x] bpftrace builtin arguments - things like `pid`, `comm`, `cpu`, `elapsed` work well, they are more or less just passed through to the bpftrace program as is. 
* [x] TUI - there's a cool TUI that let's you type in sql queries, see the bpftrace output, and then streams the results from whichever server you're targeting.
* [x] CLI - you can use '-e' to run a query on a server.
* [x] Execution - bpfquery can run a query on a server and get the results back.
* [x] There's a sick webpage that works really well and is super cool. 


# Zack's Todo's 
Ordered roughly by what I want to do next.

* [ ] Redo the data layout so that it's better more amendable to aggregation and streaming semanitcs. 
* [ ] Implement aggregation and streaming semantics.
* [ ] Make it so that the results are sent incrementally instead of all at once.
* [ ] Examples built in 
* [ ] Typeahead in the web interface both for the probes as well as the arguments 
* [ ] Use vmlinux.h to get the types of the arguments to the probes so we don't have to use -> anymore or str.  
* [ ] Have the linux kernel defs just be a big json somehow? An api endpoint for looking up defs? 
* [ ] Struct/bpf tree explorer/explainer in web interface 
* [ ] Write out docs 
* [ ] Write out some tests
* [ ] Mess around with `seluect pid from kprobe.STAR`
* [ ] Type checking and hints, see first problem query below. 


# Zack's Not Yets 

* [ ] Compiling down to bpf programs directly - hard and I want to get the semantics of sql right first, before trying to do this.

# Motivation

It took me ten years and two weeks to write my first BPF program from scratch that actually worked. I learned about eBPF back in the early 2010's and was too intimidated and flummoxed by the tooling to do anything of consequence of it. I loved the idea of getting information out of the kernel and seeing what was _really_ going on in there, however, I was never able to get past "hello world". Other projects and ideas where easier and more fruitful to do, so I did those the last decade. [Finding myself with some time during a job transition](https://www.linkedin.com/in/zack-maril/), I told myself I would finally write a BPF program after many years of putting it off and just using bpftrace. And I did! It sucked! It took two weeks of really frustrating work to just get "hello world" to run in a container on my laptop. It was not fun! 

During this time, I read a lot of programs that others had written and saw the tools that were referenced and used most often that showed the promise of BPF. This reinforced two things for me that I had felt for a long time:

1. Writing bpf programs is hard if you don't have a lot of experience with operating system internals and compilers.
2. Most bpf programs are continuous queries on streams of events coming from the kernel.

Confirmation bias aside, I thought that if I could write a SQL parser and compiler that could take a SQL query and turn it into a BPF program, I could make it easier for people (i.e. me) to write use BPF-based programs and better understand what's going on in the kernel. I had bought and read through [a book on complex event processing](https://www.amazon.com/Power-Events-Introduction-Processing-Distributed/dp/0201727897) many years ago and recently saw [ksqlDB](https://ksqldb.io/) used to great effect at my previous job, so I figured why not take a stab at it while I have some time. So far, it's been a lot of fun and I've learned a lot about SQL and BPF in the process. Maybe someday, others will find it useful too.


# License/Contributing

This project is not licensed yet and I do not know if I want outside contributions yet. It's a personal experiment and I'm more focused on experimenting than I am about licensing or others at the moment. If you have any questions, feel free to reach out to me on [twitter](https://twitter.com/zackmaril) or [linkedin](https://www.linkedin.com/in/zack-maril/).