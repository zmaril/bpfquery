

Id's are implicit 

Maps

timestamp[id] = timestamp 
field1[id] = value 
field2[id] = value
fieldN[id] = value

Every time an aggregation window closes we run the aggregateion and then collect all of the data and remove it from the map.


Identity aggregation 


----

The range of sql that bpfquery can handle right now is limited. No joins, no windows, no aggregations and only simple reporting. It's taking in streams of dataing, selecting fields from it and then printing them out. The joy and promise of sql is that we can do more than just field access across streams of data. 

Results, joins, windows, and aggregations are the heart of sql to me. There's a very simple data model underlaying bpfquery right now, just using a map for most things. 

Something like the following:

``` sql
select
      str(args.path -> dentry -> d_name.name) as filename
  from
      kprobe.vfs_open;
```

Is turned into this:

```
kprobe:vfs_open
 {
@q1_id["id"] = count();
$q1_0 = str(((struct path *)arg0) -> dentry -> d_name.name);
print((("id",@q1_id["id"]),(0,$q1_0)));
 }
```

The id isn't hardly used here at all and could be left out.


1 based indexing for now. 

How is the following supposed to work?
```
select count() from kprobe.vfs_open
```

It's a pretty simple query, I want to know how many calls to vfs_open there has been. 

In typical sql, we would just run this and be done. With data streaming in though, that's harder. There is never an end and that aggregation is always changing. 

It's a good query to run though, I like it, and can imagine the results. 

It would keep printing out the count as it increased, more or less every time that the query happened. We have an open window 


Some aggregations are expensive and we want to only happen once. 
Some aggregations are cheap and we want to happen everytime. 

select count(distinct filename) from kprobe.vfs_open. 


Gather the inputs
@q1_ids["id"] = count() or whatever it is grouped by 
@q1_inputs_v1[id] - 


```



// do set timeout 
Time limit on length? 


Design 

Every result has the following: 
* No group by then @id = count() 
* If group by, then group by results of columns, @id[...eval(columns)] = count()  more or less. 


Stages 
1. Find the id
    a. no group bys, it's just the count 
    b. if group by's, then it's the result of the group by being evaled 
2.Find the inputs coming into the query  

The groups don't have to relate to anything at all in the projections 

group_id? 



timestamp - 
status 