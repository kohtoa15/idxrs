use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::convert::TryFrom;
use std::convert::TryInto;

pub enum IdxError {
    DimensionMismatch{ needed: u8 , supplied: u8 },
    OutOfBounds{ dimension: u8, max: u32, index: u32},
    WrongHeader,
    IoError(io::Error),
    UnknownDataType,
    CannotCast,
}

/// Looks up data type and creates cursor for the type
pub fn create_idx_cursor<R: Read + Seek>(mut reader: R) -> Result<IdxCursor<R>, IdxError> {
    // Read first 4 bytes to get magic number
    let mut buf: [u8; 4] = [0; 4];
    reader.read_exact(&mut buf).map_err(|e| IdxError::IoError(e))?;

    // First two bytes must be 0
    if buf[0] != 0 || buf[1] != 0 {
        return Err(IdxError::WrongHeader);
    }

    // Read data type from third byte
    let data_type = IdxDataType::read(buf[2])?;

    // Number of dimensions are stored in fourth byte
    // Read n next numbers of dimension sizes (each 32bit)
    let n: usize = buf[3] as usize;
    let mut dimensions: Vec<u32> = Vec::with_capacity(n);
    for _i in 0..n {
        reader.read_exact(&mut buf).map_err(|e| IdxError::IoError(e))?;
        dimensions.push(u32::from_be_bytes(buf));
    }
    // Return Cursor type
    Ok(IdxCursor::from(reader, dimensions, data_type))
}

#[derive(Clone, Copy, PartialEq)]
pub enum IdxDataType {
    UnsignedByte,
    SignedByte,
    Short,
    Int,
    Float,
    Double,
}

impl IdxDataType {
    pub fn read(b: u8) -> Result<IdxDataType, IdxError> {
        match b {
            0x08 => Ok(IdxDataType::UnsignedByte),
            0x09 => Ok(IdxDataType::SignedByte),
            0x0b => Ok(IdxDataType::Short),
            0x0c => Ok(IdxDataType::Int),
            0x0d => Ok(IdxDataType::Float),
            0x0e => Ok(IdxDataType::Double),
            _ => Err(IdxError::UnknownDataType),
        }
    }

    pub fn get_size(&self) -> u8 {
        match self {
            IdxDataType::UnsignedByte => 1,
            IdxDataType::SignedByte   => 1,
            IdxDataType::Short        => 2,
            IdxDataType::Int          => 4,
            IdxDataType::Float        => 4,
            IdxDataType::Double       => 8,
        }
    }

    pub fn create_buf(&self) -> Box<[u8]> {
        match self {
            IdxDataType::UnsignedByte => Box::new([0; 1]),
            IdxDataType::SignedByte   => Box::new([0; 1]),
            IdxDataType::Short        => Box::new([0; 2]),
            IdxDataType::Int          => Box::new([0; 4]),
            IdxDataType::Float        => Box::new([0; 4]),
            IdxDataType::Double       => Box::new([0; 8]),
        }
    }
}

macro_rules! from_slice {
    // ($T:ty, $src:tt) => { $T::from_be_bytes($src.try_into().map_err(|_| IdxError::CannotCast)?) };
    ($T:ty, $src:expr) => { 
        $src.try_into()
        .map(|b| <$T>::from_be_bytes(b))
        .map_err(|_| IdxError::CannotCast)
    };
}

pub enum IdxValue {
    UnsignedByte(u8),
    SignedByte(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    Double(f64),
}

impl TryFrom<(IdxDataType, Box<[u8]>)> for IdxValue {
    type Error = IdxError;
    fn try_from(tuple: (IdxDataType, Box<[u8]>)) -> Result<IdxValue, Self::Error> {
        let (idt, bytes) = tuple;
        if idt.get_size() as usize != bytes.len() {
            return Err(IdxError::CannotCast);
        }
        let val = match idt {
            IdxDataType::UnsignedByte => IdxValue::UnsignedByte(from_slice!(u8, &bytes[..])?),
            IdxDataType::SignedByte   => IdxValue::SignedByte(from_slice!(i8, &bytes[..])?),
            IdxDataType::Short        => IdxValue::Short(from_slice!(i16, &bytes[..])?),
            IdxDataType::Int          => IdxValue::Int(from_slice!(i32, &bytes[..])?),
            IdxDataType::Float        => IdxValue::Float(from_slice!(f32, &bytes[..])?),
            IdxDataType::Double       => IdxValue::Double(from_slice!(f64, &bytes[..])?),
        };
        Ok(val)
    }
}

pub struct IdxCursor<R: Read + Seek> {
    reader: R,
    dimensions: Vec<u32>,
    data_type: IdxDataType,
}

impl<R: Read + Seek> IdxCursor<R> {
    pub fn from(reader: R, dimensions: Vec<u32>, data_type: IdxDataType) -> IdxCursor<R> {
        IdxCursor {
            reader, dimensions, data_type
        }
    }

    pub fn get(&mut self, indices: &[u32]) -> Result<(IdxDataType, Box<[u8]>), IdxError> {
        // Throw index error if index parameter does not fit dimension count
        if indices.len() != self.dimensions.len() {
            return Err(IdxError::DimensionMismatch{ needed: self.dimensions.len() as u8, supplied: indices.len() as u8 });
        }
        // Check indices and sizes of individual dimensions
        for (i, (dimension, index)) in self.dimensions.iter().zip(indices.iter()).enumerate() {
            if index >= dimension {
                return Err(IdxError::OutOfBounds{ dimension: i as u8, max: *dimension, index: *index });
            }
        }
        // seek to correct spot and return data
        let mut pos: u64 = 0;
        let mut mult: u64 = 1;
        for (dimension, index) in self.dimensions.iter().rev().zip(indices.iter().rev()) {
            mult *= *dimension as u64;
            pos += *index as u64 * mult;
        }
        // Manipulate position by data type intervals and header size
        pos *= self.data_type.get_size() as u64;
        // Header size = 4B + 4B * dimensions
        pos += 4 + 4 * self.dimensions.len() as u64;
        let _res = self.reader.seek(SeekFrom::Start(pos)).map_err(|e| IdxError::IoError(e))?;
        let mut buffer = self.data_type.create_buf();
        self.reader.read_exact(&mut buffer).map_err(|e| IdxError::IoError(e))?;
        Ok((self.data_type, buffer))
    }
}

