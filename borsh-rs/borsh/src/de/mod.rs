use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Error, Read};
use std::mem::{forget, size_of};

mod hint;

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";

/// A data-structure that can be de-serialized from binary format by NBOR.
pub trait BorshDeserialize: Sized {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error>;

    /// Deserialize this instance from a slice of bytes.
    fn try_from_slice(v: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor::new(v);
        let result = Self::deserialize(&mut c)?;
        if c.position() != v.len() as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ERROR_NOT_ALL_BYTES_READ,
            ));
        }
        Ok(result)
    }
}

impl BorshDeserialize for () {
    fn deserialize<R: Read>(_reader: &mut R) -> Result<Self, Error> {
        Ok(())
    }
}

impl BorshDeserialize for u8 {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut res = 0u8;
        reader.read_exact(std::slice::from_mut(&mut res))?;
        Ok(res)
    }
}

macro_rules! impl_for_integer {
    ($type: ident) => {
        impl BorshDeserialize for $type {
            #[inline]
            fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
                let mut data = [0u8; size_of::<$type>()];
                reader.read_exact(&mut data)?;
                Ok($type::from_le_bytes(data))
            }
        }
    };
}

impl_for_integer!(i8);
impl_for_integer!(i16);
impl_for_integer!(i32);
impl_for_integer!(i64);
impl_for_integer!(i128);
impl_for_integer!(u16);
impl_for_integer!(u32);
impl_for_integer!(u64);
impl_for_integer!(u128);

// Note NaNs have a portability issue. Specifically, signalling NaNs on MIPS are quiet NaNs on x86,
// and vice-versa. We disallow NaNs to avoid this issue.
macro_rules! impl_for_float {
    ($type: ident, $int_type: ident) => {
        impl BorshDeserialize for $type {
            fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
                let mut data = [0u8; size_of::<$type>()];
                reader.read_exact(&mut data)?;
                let res = $type::from_bits($int_type::from_le_bytes(data));
                if res.is_nan() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "For portability reasons we do not allow to deserialize NaNs.",
                    ));
                }
                Ok(res)
            }
        }
    };
}

impl_for_float!(f32, u32);
impl_for_float!(f64, u64);

impl BorshDeserialize for bool {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] == 1)
    }
}

impl<T> BorshDeserialize for Option<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut flag = [0u8];
        reader.read_exact(&mut flag)?;
        if flag[0] == 0 {
            Ok(None)
        } else {
            Ok(Some(T::deserialize(reader)?))
        }
    }
}

impl<T, E> BorshDeserialize for Result<T, E>
where
    T: BorshDeserialize,
    E: BorshDeserialize,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut flag = [0u8];
        reader.read_exact(&mut flag)?;
        Ok(if flag[0] == 0 {
            Ok(T::deserialize(reader)?)
        } else {
            Err(E::deserialize(reader)?)
        })
    }
}

impl BorshDeserialize for String {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = u32::deserialize(reader)?;
        // TODO(16): return capacity allocation when we have the size of the buffer left from the reader.
        let mut result = Vec::with_capacity(hint::cautious::<u8>(len));
        for _ in 0..len {
            result.push(u8::deserialize(reader)?);
        }
        String::from_utf8(result)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))
    }
}

#[cfg(feature = "std")]
impl<T> BorshDeserialize for Vec<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = u32::deserialize(reader)?;
        if size_of::<T>() == 0 {
            let mut result = Vec::new();
            result.push(T::deserialize(reader)?);

            let p = result.as_mut_ptr();
            unsafe {
                forget(result);
                let len = len as usize;
                let result = Vec::from_raw_parts(p, len, len);
                Ok(result)
            }
        } else {
            // TODO(16): return capacity allocation when we can safely do that.
            let mut result = Vec::with_capacity(hint::cautious::<T>(len));
            for _ in 0..len {
                result.push(T::deserialize(reader)?);
            }
            Ok(result)
        }
    }
}

#[cfg(feature = "std")]
impl<T, S> BorshDeserialize for HashSet<T, S>
where
    T: BorshDeserialize + Eq + std::hash::Hash,
    S: std::hash::BuildHasher + Default,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let vec = <Vec<T>>::deserialize(reader)?;
        Ok(vec.into_iter().collect())
    }
}

#[cfg(feature = "std")]
impl<K, V, S> BorshDeserialize for HashMap<K, V, S>
where
    K: BorshDeserialize + Eq + std::hash::Hash,
    V: BorshDeserialize,
    S: std::hash::BuildHasher + Default,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = u32::deserialize(reader)?;
        // TODO(16): return capacity allocation when we can safely do that.
        let mut result = HashMap::default();
        for _ in 0..len {
            let key = K::deserialize(reader)?;
            let value = V::deserialize(reader)?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

#[cfg(feature = "std")]
impl<T> BorshDeserialize for BTreeSet<T>
where
    T: BorshDeserialize + Ord,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let vec = <Vec<T>>::deserialize(reader)?;
        Ok(vec.into_iter().collect())
    }
}

#[cfg(feature = "std")]
impl<K, V> BorshDeserialize for BTreeMap<K, V>
where
    K: BorshDeserialize + Ord,
    V: BorshDeserialize,
{
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = u32::deserialize(reader)?;
        let mut result = BTreeMap::new();
        for _ in 0..len {
            let key = K::deserialize(reader)?;
            let value = V::deserialize(reader)?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddr {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let kind = u8::deserialize(reader)?;
        match kind {
            0 => std::net::SocketAddrV4::deserialize(reader).map(std::net::SocketAddr::V4),
            1 => std::net::SocketAddrV6::deserialize(reader).map(std::net::SocketAddr::V6),
            value => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid SocketAddr variant: {}", value),
            )),
        }
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddrV4 {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let ip = std::net::Ipv4Addr::deserialize(reader)?;
        let port = u16::deserialize(reader)?;
        Ok(std::net::SocketAddrV4::new(ip, port))
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddrV6 {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let ip = std::net::Ipv6Addr::deserialize(reader)?;
        let port = u16::deserialize(reader)?;
        Ok(std::net::SocketAddrV6::new(ip, port, 0, 0))
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::Ipv4Addr {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(std::net::Ipv4Addr::from(buf))
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::Ipv6Addr {
    #[inline]
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf)?;
        Ok(std::net::Ipv6Addr::from(buf))
    }
}

impl BorshDeserialize for Box<[u8]> {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = u32::deserialize(reader)?;
        // TODO(16): return capacity allocation when we can safely do that.
        let mut result = Vec::with_capacity(hint::cautious::<u8>(len));
        for _ in 0..len {
            result.push(u8::deserialize(reader)?);
        }
        Ok(result.into_boxed_slice())
    }
}

macro_rules! impl_arrays {
    ($($len:expr => ($($n:expr)+))+) => {
        $(
            impl<T: BorshDeserialize> BorshDeserialize for [T; $len] {
                #[inline]
                fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
                    // As byte arrays are packed in borsh, this generic implementation should
                    // produce the same code as an unrolled `reader.read_exact($len)`.
                    Ok([$(
                        T::deserialize(reader)
                        .map_err(|e|
                            Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("error deserializing element at index {}: {}", $n, e)
                            )
                        )?
                    ),+])
                }
            }
        )+
    };
}

impl<T: BorshDeserialize> BorshDeserialize for [T; 0] {
    fn deserialize<R: Read>(_reader: &mut R) -> Result<Self, Error> {
        Ok([])
    }
}

oasis_borsh_derive::_gen_seq_macro! {
    impl_arrays => (1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 32 64 65)
}

macro_rules! impl_tuples {
    ($($len:literal => ($($name:ident)+))+) => {
        $(
            impl<$($name: BorshDeserialize),+> BorshDeserialize for ($($name),+) {
                fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
                    Ok(($($name::deserialize(reader)?,)+))
                }
            }
        )*
    }
}

oasis_borsh_derive::_gen_seq_macro! {
    impl_tuples => T :: (2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19)
}
