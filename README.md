# database
**database** is a network and serialized key-value store written in Rust. My aim in building this project was to learn how Rust and databases work, and to build skills in networking and multithreading. **THIS IS A LEARNING PROJECT ONLY! DO NOT USE THIS AS AN ACTUAL DATABASE!**

## How to Run it
In order to run this project, you need to have Go and Rust installed.

Firstly, clone the git repository with `git clone https://github.com/kawiggles/database.git`, and then `cd` into it.

Run `cargo build --release` in order to build the executable, which can be run inside the directory as with `./target/release/database`. Running this executable will create a log file and database file in the directory that you run it. 

There are three basic commands corresponding with the typical commands of a key-value store: `get`, `set`, and `del`. Enter `help` for more information on syntax, and `stop` to stop the server.

## Architecture and Design
This is a fairly typical database, essentially copying directly from MySQL in the sense that it uses a b+ tree and fixed page sizes. I did this because it was easier than learning a complex architecture on top of a new programming language and a bunch of new programming concepts. A core decision was to reduce the number of external dependencies so that I could learn what those dependencies were abstracting away. The nonstandard crates I use right now are:
+ bincode-next
+ log/simplelog
+ tempfile
+ thiserror
+ ctrlc
+ tokio
tokio, thiserror, and log/simplelog are likely to be permanent inclusions. tempfile is used only for testing, and bincode-next will eventually be depreciated in favor of a more efficient custom encoding and decoding system.

### Organizational Data Structure
The core organizational data structure of the database is a b+ tree. Instead of using nodes with pointers, like you would in C++ or Go, I used an arena allocator which started out as a basic vector of nodes and was eventually replaced by the pager. The idea is that the "pointers" to the nodes are actually just indexes in the vector, which allows quick r/w access to the nodes. I did this because actual pointers are really difficult in Rust, and trying to make a traditional "C-like" b+ tree would result in entirely too many instances of `Option<Rc<RefCell<Node>>>`. The b+ tree being arena allocated made it extremely easy to merge with the pager, as node indices in the vector of nodes was simply replaced by page indexes in concert with pager read and write operations.

The tree has three basic operations, matching those of the hash map which the tree replaced: get, insert, and remove. Implementing the insert and remove operations was by far the hardest part of this project so far because of the number of edge cases in each operation. Off by one errors were abound, and the tree structure itself is difficult to visualize. In the bptree.rs file, you'll find a validator I wrote for testing the tree as well as methods for printing out the tree, which I used heavily to debug the structure. You can now also run the `validate` command or the `print tree` command.

### Pager Architecture
The pager architecture was a lot of fun to write because it deals with fixed binary buffers and read/write operations in concert with an in-memory data structure. Getting the two to coordinate was an interesting challenge.

The database is contained in one file, which is split into a number of pages. Currently, a page is 4KB (4096 bytes), but I plan on making this value mutable. Pages are referenced by their PageId, which is an integer value corresponding with the page's offset, in multiples of 4096 bytes, from the start of the database document. There are two kinds of pages.

The first is an IndexPage, which is what replaced the nodes of the b+ tree. There are two kinds of IndexPages: Leaves and Branches. Branches contain a vector of keys and a vector of associated child values, which are PageIds for other IndexPages. Leaves contain a similar vector of keys, but their associated vector is a vector of PageIds for DataPages. Given the size of 4096 bytes, each IndexPage can hold about 150 keys, making the order of the default b+ tree 150.

DataPages are the second kind of page, and these are the pages that hold the raw data. They consist only of a short header and the raw data, which is a enum called Value. Value allows for four data types: strings, integers, floats, and blobs. Blobs will be the means by which files will be encoded in the future. DataPages currently are limited to 4KB entries, but I have a plan to remove this storage limitation.

The pager itself is essentially a struct that holds the database filepath, metadata, and a suite of methods to expose pages to the b+ tree for read write operations. Critically, the pager writes changes by adding all the modified pages to a hash map from their page indices, and then writes all the updated pages once the database operation is complete. The pager also keeps tracks of pages which have been deleted so that they can be reallocated, preventing the database from using unnecessary extra space.

The pager currently uses bincode to read and write raw data to storage, but I plan on eventually implementing my own system for encoding data to save space (doubling the number of keys that can be placed in an IndexPage) and reduce dependencies.

### Networking
Networking is accomplished through the tokio `net`crate. The server uses tokio to start a thread for the local cli, and then creates a new thread for each connection that it receives. A RwLock allows multiple clients to interact with the database simultaneously. The server is abstracted as a struct with methods that wrap the initialization of the TCP socket and the local database as well as handle client connections.

The server has been set up to use the postgresql protocol, meaning that prebuilt tools like psql work with the server. However, I haven't implemented SQL yet, so you have to use my weird GET/SET/DEL syntax. That will come later. Also the protocol super isn't built out, so you can really only input and output values you enter manually. Work on this coming. 

## Project Details
### History of the Project
The project has slowly added features as I've learned about Rust. Here's a rough timeline of progress I've made on the project:
+ 6/3/26: project started
+ 6/5/26: basic key-value store with hash map implemented
+ 6/9/26: b+ tree implementation
+ 6/14/26: pager implemented
+ 6/19/26: basic networking implementation
+ 6/21/26: basic multithreading implementation 
+ 7/4/26: basic version of postgresql protocol
+ 7/26/26: tokio async implementation

### Known Bugs
+ \q with psql does not work, as far as I can tell
+ a tokio thread failing messes up the local cli
+ a BUNCH of postgresql networking isn't being handles, specifically error messages

### Future Plans
Because this project is for learning, it will be under constant, slow development for pretty much the entirety of its existence. I aim to add features that will help me learn about programming and systems architecture. If I'm lucky, this project may one day end up as a functional database. Below is a list of features I plan to implement at some point in the future, in order of priority (I don't know how to do abstract syntax trees yet).
+ Increase max value size from 4KB to whatever size on disk is necessary
    + Modify wire protocol to process values in chunks
    + Write DataPage overflow chains
    + Edit Go client to read and create files for storing and extracting values
+ Modify Pager
    + Remove bincode dependency and encode pages directly
    + Make file pages slotted for memory efficiency. 
    + Move async operations from the whole store to individual pages
+ Improve resiliency
    + Do an error handling overhaul
        + Update error handling messages
        + Unify failure modes for categories of errors
    + Write-Ahead-Logging for ACID compliance
    + Better logging throughout project
+ Add configuration file and parser with serde, configure page size, server port, and db file path
+ Add a SQL parser
    + Add lexer
    + Add AST builder
    + add execution planner
