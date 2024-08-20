# bpfquery

An experiment with compiling SQL to BPF(trace) programs. 

```bash
git clone git@github.com:zmaril/bpfquery.git
cd bpfquery
cargo install --path .
bpfquery devserver #some linux server you have ssh access to that has bpftrace installed on it 
> select pid, cpu, elapsed from kprobe.do_nanosleep;
# watch as bpftrace sends info back about things
```

# Design/Roadmap 

Right now bpfquery is (sort of) working. It's got a TUI and CLI that (sort of works), a sql to bpftrace compiler that (sort of) works, and an executor that (sort of) works. What's next up is removing those (sort of) qualifiers and making it just work. 

The main focus right now is expanding the SQL to BPF compiler to handle more SQL queries, with incidental improvements to the UI and executor as needed. Joins don't work, only the builtin bpftrace arguments like pid and comm are supported, and there's no streaming semantics yet. So figuring out how to make those work is the next step. Much later on, after I've nailed down the semantics of the language, I'd like to make it so that the backend can be switched out for other BPF backends like libbpf or bcc. But there's a lot of experimentation to do before that happens.

# Related Work

* [bpftrace](https://github.com/bpftrace/bpftrace)
* ebql - [code](https://github.com/ringtack/ebql) and [paper](https://etos.cs.brown.edu/publications/theses/rtang-honors.pdf)

# Progress so far 

* [x] Expressions - a lot of expressions just work so far, but there's a lot of edge cases to handle to as they come up, but the expectation is that something like `select pid + 1 from kprobe.do_nanosleep` should work.
* [x] Predicates/filtering/`where` - `where` clauses get parsed and compiled into predicates and often work. `select * from kprobe.do_nanosleep where pid > 1000` should work fine.
* [x] bpftrace builtin arguments - things like `pid`, `comm`, `cpu`, `elapsed` work well, they are more or less just passed through to the bpftrace program as is. 
* [x] TUI - there's a cool TUI that let's you type in sql queries, see the bpftrace output, and then streams the results from whichever server you're targeting.
* [x] CLI - you can use '-e' to run a query on a server.
* [x] Execution - bpfquery can run a query on a server and get the results back.


# Zack's Todo's 
Ordered roughly by what I want to do next.

* [ ] Explore streaming joins across tables.
* [ ] Explore streaming semantics with UDF window like arroyo.dev
* [ ] Explore putting charts in the TUI and seeing if that's useful.
* [ ] Typing in the TUI is slow sometimes and hangs, unsure why.
* [ ] Type checking and hints, see first problem query below. 
* [ ] Try out other probes besides kprobe
* [ ] Figure out how bpftrace name star will work, i.e. `select pid from kprobe.*` equivalent.
* [ ] Experiment with static table access, like looking things up about the os before hand in a table. 
* [ ] Experiment with args from `tracepoint`, `kfunc`, and `uprobe` 
* [ ] Write some tests, starting to get tough to keep track of what works and what doesn't. 
* [ ] Set up CI pipelines with releases 
* [ ] Make the TUI sections (table, editor) all scrollable 
* [ ] Expand the TUI to have like menus and stuff.
* [ ] Put some examples into the TUI premade for people to try out 
* [ ] Have a bpf struct and bpftrace probe tree explorer/explainer so people can see what's available to them.
* [ ] Use the output of vmlinux.h and bpftrace.lv.txt somehow to make the bpftrace program more robust, combined with static type checking or something, but also just knowing ahead of time whether a tracepoint exists 
* [ ] Write some docs about how to use everything, what can be expected to work. 


# Problem Queries 

```sql
//stdin:1:26-27: WARNING: comparison of integers of different signs: 'unsigned int64' and 'int64' can lead to undefined behavior
select * from kprobe.do_nanosleep where pid > 2*1000
```


# Motivation

It took me ten years and two weeks to write my first BPF program from scratch that actually worked. I learned about eBPF back in the early 2010's and was too intimidated and flummoxed by the tooling to do anything of consequence of it. I loved the idea of getting information out of the kernel and seeing what was _really_ going on in there, however, I was never able to get past "hello world". Other projects and ideas where easier and more fruitful to do, so I did those the last decade. [Finding myself with some time during a job transition](https://www.linkedin.com/in/zack-maril/), I told myself I would finally write a BPF program after many years of putting it off and just using bpftrace. And I did! It sucked! It took two weeks of really frustrating work to just get "hello world" to run in a container on my laptop. It was not fun! 

During this time, I read a lot of programs that others had written and saw the tools that were referenced and used most often that showed the promise of BPF. This reinforced two things for me that I had felt for a long time:

1. Writing bpf programs is hard if you don't have a lot of experience with operating system internals and compilers.
2. Most bpf programs are continuous queries on streams of events coming from the kernel.

Confirmation bias aside, I thought that if I could write a SQL parser and compiler that could take a SQL query and turn it into a BPF program, I could make it easier for people (i.e. me) to write use BPF-based programs and better understand what's going on in the kernel. I had bought and read through [a book on complex event processing](https://www.amazon.com/Power-Events-Introduction-Processing-Distributed/dp/0201727897) many years ago and recently saw [ksqlDB](https://ksqldb.io/) used to great effect at my previous job, so I figured why not take a stab at it while I have some time. So far, it's been a lot of fun and I've learned a lot about SQL and BPF in the process. Maybe someday, others will find it useful too.


# License/Contributing

This project is not licensed yet and I do not know if I want outside contributions yet. It's a personal experiment and I'm more focused on experimenting than I am about licensing or others at the moment. If you have any questions or want to talk about it, feel free to reach out to me on [twitter](https://twitter.com/zackmaril) or [linkedin](https://www.linkedin.com/in/zack-maril/).