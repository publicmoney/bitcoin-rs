# Hammersbald
A fast embedded blockchain database.

## Motivation
Generic databases and key-value stores offer much more functionality 
than needed to store and process a blockchain. Superfluous functionality (for a blockchain)
comes at a high cost in speed. 

## Name
Hammersbald is a German slang for "Haben wir es bald?" close to in english "Will we have it soon?". 
A term often used to express impatience. Hammersbald is the blockchain database for the impatient.
Hammersbald sounds also like the name of some ancient northern god.

## Design

A Hammersbald database consists of four file groups. A file group consists files of max. 1 GiB size each.

* Hash table slots. These are updated in random order as needed. (*.tb).  
* A hash table slot points to a list of data pointers. Data pointers are stored in an append only file group. (*.bl).    
* The blockchain data is stored in an append only file group (*.bc).  
* Log is a temporary store of hash table slot pre-images (.lg).  

The hash table is updated an consulted at high frequency therefore a copy of it is also maintained in memory. The number of slots is limited to 2³². A slot may point to any number of data elements, the mean length of the data pointer lists is configurable, but the actual length of a list is probabilistic. Hammersbald uses SipHash with a random key to achieve approximately even distribution of data to slots.  

The hash table is written to disk at the end of a batch that can bundle any number of inserts.  

Hammersbald recovers gracefully from a crash of the process it is running in: It stores pre-images of the hash table slots and sizes of the append only files in the log before changing the persistent hash table. It checks at start the log file, that might be left over from a crashed process, patches the hash table to its last known consistent state with the pre-images and truncate the append only stores to their last known correct size. In effect the last batch is either committed in its entirety or will be rolled back at startup.  

Writes to append only files are asynchronous. Since hash table slots and links are cached in memory, read access to keyed data takes at most one disk seek.  

Iteration is sequential in the reverse order of data inserts and skips over unreferenced data. Sequential search with iterators will be dominated by the speed of the data parser, not Hammersbald.  

## API
This library implements the bare minimum of operations:

* insert data with a key
* find data with a key
* insert some data that can be referred to by an other data but has no key.
* find some data with known offset.
* start batch, that also ends current batch

There is no delete operation. An insert with a key renders a previous insert with same key inaccessible. 
Keys are not sorted and can not be iterated. 
 
Inserts must be grouped into batches. All inserts of a batch will be stored 
or none of them, in case the process dies while inserting in a batch.

Data inserted in a batch may be fetched before closing the batch.


## Implementation
The persistent storage should be opened by only one process. 

The store is a persistent hash map using [Linear Hashing](https://en.wikipedia.org/wiki/Linear_hashing).

### Limits
The data storage size is limited to 2^48 (256TiB) due to the use of 6 byte persistent
pointers. A data element can not exceed 2^24 (16MiB) in length. Key length is limited to 255 bytes. 

### History
Originally created by Tamas Blummer in 2018.  
Cloned from https://github.com/rust-bitcoin/hammersbald into this project in 2020. 