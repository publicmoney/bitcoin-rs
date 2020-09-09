[![Safety Dance](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

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

## Status
It works.

## Design
See [Hammersbald](https://medium.com/@tamas.blummer/hammersbald-7c0bda14da1e) on medium

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

Simplest use:
````$Rust
use hammersbald::{
        persistent,
        HammersbaldAPI
        };

// read cache size in 4k pages
const CACHED_PAGES:usize = 100;
// average re-use of a hash table entry
const BUCKET_FILL_TARGET:usize = 2;

let mut db = persistent("dbname", CACHED_PAGES, BUCKET_FILL_TARGET).unwrap();

db.put_keyed(b"some key", b"some data").unwrap();

db.batch().unwrap();

if let Some((pos, data)) = db.get_keyed(b"some key").unwrap() {
    assert_eq!(data, b"some data".to_vec());
}
else {
    panic!("can not find inserted data");
}


db.shutdown();
````

## Implementation
The persistent storage should be opened by only one process. 

The store is a persistent hash map using [Linear Hashing](https://en.wikipedia.org/wiki/Linear_hashing).

### Limits
The data storage size is limited to 2^48 (256TiB) due to the use of 6 byte persistent
pointers. A data element can not exceed 2^24 (16MiB) in length. Key length is limited to 255 bytes. 

## Release Notes
2.3.0 all bitcoin objects use CBOR serialization

2.2.0 add storage of CBOR serializable objects to bitcoin_adaptor

2.1.0 upgrade to rust-bitcoin 0.20, use bitcoin_hashes instead of siphasher

2.0.0 file format change, some savings

1.7.0 group subsequent reads and writes, upgrade to rust-bitcoin 0.18

1.6.0 upgrade to rust-bitcoin 0.17

1.5.1 add API may_have_key

1.5 upgrade to bitcoin 0.16

