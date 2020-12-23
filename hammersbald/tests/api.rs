extern crate rand;

use self::rand::thread_rng;
use hammersbald::Error;
use hammersbald::{persistent, transient};
use rand::RngCore;
use std::collections::HashMap;

#[test]
fn test_two_batches_indexed() {
	let path = "testdb/bat";
	std::fs::remove_dir_all(path).unwrap_or_default();

	let mut rng = thread_rng();
	let mut check = HashMap::new();
	let mut key = [0x0u8; 32];
	let mut data = [0x0u8; 40];

	{
		let mut db = persistent(path, "test", 1).unwrap();
		for _ in 0..100 {
			rng.fill_bytes(&mut key);
			rng.fill_bytes(&mut data);
			let pref = db.put_keyed(&key, &data).unwrap();
			check.insert(key, (pref, data));
		}
		db.batch().unwrap();

		for _ in 0..100 {
			rng.fill_bytes(&mut key);
			rng.fill_bytes(&mut data);
			let pref = db.put_keyed(&key, &data).unwrap();
			check.insert(key, (pref, data));
		}
		db.batch().unwrap();

		for (k, (pref, v)) in check.iter() {
			assert_eq!(db.get(pref.clone()).unwrap(), (k.to_vec(), v.to_vec()));
			assert_eq!(db.get_keyed(&k[..]).unwrap(), Some((*pref, v.to_vec())));
		}
		db.shutdown().unwrap();
	}

	{
		// Now reopen database and check again.
		let mut db = persistent(path, "test", 1).unwrap();

		for (k, (pref, v)) in check.iter() {
			assert_eq!(db.get(pref.clone()).unwrap(), (k.to_vec(), v.to_vec()));
			assert_eq!(db.get_keyed(&k[..]).unwrap(), Some((*pref, v.to_vec())));
		}
		db.shutdown().unwrap();
	}
}

#[test]
fn test_put_get() {
	let mut db = transient().unwrap();

	let value = [1, 2, 3];
	let pref = db.put(&value).unwrap();
	assert_eq!(0, pref);

	let result = db.get(pref).unwrap();

	assert_eq!(0, result.0.len());
	assert_eq!(value.to_vec(), result.1)
}

#[test]
fn test_put_get_big() {
	let mut db = transient().unwrap();

	let value = [5u8; 5000];
	let pref = db.put(&value).unwrap();
	assert_eq!(0, pref);

	let result = db.get(pref).unwrap();

	assert_eq!(0, result.0.len());
	assert_eq!(value.to_vec(), result.1)
}

#[test]
fn test_put_keyed_then_set_same_length() {
	let mut db = transient().unwrap();

	let key = "abc";
	let value = [1, 2, 3];
	let new_value = [4, 5, 6];

	let pref = db.put_keyed(key.as_ref(), &value).unwrap();
	let result = db.set(pref, &new_value).unwrap();
	db.batch().unwrap();

	assert_eq!(result, pref);
	assert_eq!(db.get(pref).unwrap().1, new_value);
	assert_eq!(db.get_keyed(key.as_ref()).unwrap().unwrap().1, new_value)
}

#[test]
fn test_put_keyed_then_set_different_length_returns_error() {
	let mut db = transient().unwrap();
	let key = "abc";
	let value = [1, 2, 3];
	let new_value = [4, 5, 6, 7];

	let pref = db.put_keyed(key.as_ref(), &value).unwrap();
	let result = db.set(pref, &new_value);
	db.batch().unwrap();

	assert_eq!(result.err().unwrap().to_string(), Error::ValueTooLong.to_string());
}

#[test]
fn test_put_then_set_different_length_returns_error() {
	let mut db = transient().unwrap();
	let value = [1, 2, 3];
	let new_value = [4, 5, 6, 7];

	let pref = db.put(&value).unwrap();
	let result = db.set(pref, &new_value);
	db.batch().unwrap();

	assert_eq!(result.err().unwrap().to_string(), Error::ValueTooLong.to_string());
}

#[test]
fn test_put_then_set() {
	let mut db = transient().unwrap();
	let value = [1, 2, 3];
	let new_value = [4, 5, 6];

	let pref = db.put(&value).unwrap();
	let result = db.set(pref, &new_value).unwrap();
	db.batch().unwrap();

	assert_eq!(db.get(result).unwrap().0, vec![]);
	assert_eq!(db.get(result).unwrap().1, new_value);
}

#[test]
fn test_put_keyed_same_length() {
	let mut db = transient().unwrap();
	let key = "abc";
	let value = [1, 2, 3];
	let new_value = [4, 5, 6];

	let pref1 = db.put_keyed(key.as_ref(), &value).unwrap();
	let pref2 = db.put_keyed(key.as_ref(), &new_value).unwrap();

	assert_eq!(pref1, pref2);
	assert_eq!(db.get_keyed(key.as_ref()).unwrap().unwrap().1, new_value);
	db.batch().unwrap();
	assert_eq!(db.get_keyed(key.as_ref()).unwrap().unwrap().1, new_value)
}

#[test]
fn test_put_keyed_different_length() {
	let mut db = transient().unwrap();
	let key = "abc";
	let value = [1, 2, 3];
	let new_value = [4, 5, 6, 7];

	let pref1 = db.put_keyed(key.as_ref(), &value).unwrap();
	let pref2 = db.put_keyed(key.as_ref(), &new_value).unwrap();

	assert_ne!(pref1, pref2);
	assert_eq!(db.get_keyed(key.as_ref()).unwrap().unwrap().1, new_value);
	db.batch().unwrap();
	assert_eq!(db.get_keyed(key.as_ref()).unwrap().unwrap().1, new_value)
}
