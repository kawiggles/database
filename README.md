## database
**database** is a serialized key value store written in Rust. It makes use of a B+ tree to sort and search for data, and bincode-next to serialize the data into long-term storage. I have eventual plans to add a more robust storage engine, sql parser, execution planner, and networking so that it can function as a complete database server.
