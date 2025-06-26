use std::fmt::Debug;

use nfs_mamont::xdr::{deserialize, Deserialize, Serialize};

#[derive(Default)]
struct Context {
    buf: Vec<u8>,
}

trait TestValue: Deserialize + Serialize + Eq + Default + Debug + Clone {}
impl<T: Deserialize + Serialize + Eq + Default + Debug + Clone> TestValue for T {}

impl Context {
    fn check<T: TestValue>(&mut self, src_value: &T) {
        for capacity in 0..32 {
            for exsist in 0..capacity {
                self.buf = Vec::with_capacity(capacity);
                self.buf.resize(exsist, Default::default());

                src_value.serialize(&mut self.buf).expect("cannot serialize");
                assert_eq!((self.buf.len() - exsist) % 4, 0);

                let result_value =
                    deserialize::<T>(&mut &self.buf[exsist..]).expect("cannot deserialize");

                assert_eq!(src_value, &result_value);
            }
        }
    }

    fn check_multi<T: TestValue>(&mut self, src_values: &[T]) {
        src_values.iter().for_each(|i| self.check(i));
    }
}

#[derive(Default, PartialEq, Eq, Debug, Clone)]
struct TestForVecU8(Vec<u8>);

impl Serialize for TestForVecU8 {
    fn serialize<W: std::io::Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.0.serialize(dest)
    }
}

impl Deserialize for TestForVecU8 {
    fn deserialize<R: std::io::Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        self.0.deserialize(src)
    }
}

#[derive(Default, PartialEq, Eq, Debug, Clone)]
struct TestForVec<T>(Vec<T>);

impl<T: TestValue> Serialize for TestForVec<T> {
    fn serialize<W: std::io::Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.0.serialize(dest)
    }
}

impl<T: TestValue> Deserialize for TestForVec<T> {
    fn deserialize<R: std::io::Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        self.0.deserialize(src)
    }
}

#[derive(Default, PartialEq, Eq, Debug, Clone)]
struct TestForString(String);

impl Serialize for TestForString {
    fn serialize<W: std::io::Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.0.serialize(dest)
    }
}

impl Deserialize for TestForString {
    fn deserialize<R: std::io::Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        self.0.deserialize(src)
    }
}

#[test]
fn test_scalar_bijection() {
    let mut ctx = Context::default();

    ctx.check_multi(&[true, false]);

    ctx.check_multi(&[i32::MIN, -1i32, 0i32, 1i32, i32::MAX]);
    ctx.check_multi(&[i64::MIN, -1i64, 0i64, 1i64, i64::MAX]);

    ctx.check_multi(&[u32::MIN, 0u32, 1u32, 2u32, u32::MAX]);
    ctx.check_multi(&[u64::MIN, 0u64, 1u64, 2u64, u64::MAX]);
}

#[test]
fn test_array_bijection() {
    let mut ctx = Context::default();

    ctx.check(&[1u8]);
    ctx.check(&[1u8, 2u8, 3u8]);
    ctx.check(&[1u8, 2u8, 3u8, 4u8, 5u8, 6u8]);

    ctx.check(&[0u32]);
    ctx.check(&[1u32, 2u32, 3u32]);
    ctx.check(&[1u64, 2u64, 3u64]);
    ctx.check(&[1u64, 2u64, 3u64, 4u64]);
    ctx.check(&[1u64, 2u64, 3u64, 4u64, 5u64]);
}

#[test]
fn test_str_bijection() {
    let mut ctx = Context::default();

    ctx.check_multi(&[
        TestForString(String::from("")),
        TestForString(String::from("abc1234+-")),
        TestForString(String::from("abc")),
    ]);
}

#[test]
fn test_vec_bijection() {
    let mut ctx = Context::default();

    ctx.check_multi(&[
        TestForVecU8(vec![]),
        TestForVecU8(vec![1u8]),
        TestForVecU8(vec![1u8, 2u8, 3u8]),
        TestForVecU8(vec![1u8, 2u8, 3u8, 4u8]),
    ]);
    ctx.check_multi(&[
        TestForVec(vec![]),
        TestForVec(vec![1u32]),
        TestForVec(vec![1u32, 2u32, 3u32]),
        TestForVec(vec![1u32, 2u32, 3u32, 4u32]),
    ]);
    ctx.check_multi(&[
        TestForVec(vec![]),
        TestForVec(vec![1u64]),
        TestForVec(vec![1u64, 2u64, 3u64]),
        TestForVec(vec![1u64, 2u64, 3u64, 4u64]),
    ]);
}
