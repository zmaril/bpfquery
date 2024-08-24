

Id's are implicit 

Maps

timestamp[id] = timestamp 
field1[id] = value 
field2[id] = value
fieldN[id] = value

Every time an aggregation window closes we run the aggregateion and then collect all of the data and remove it from the map.


Identity aggregation 