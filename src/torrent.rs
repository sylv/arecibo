use bendy::decoding::{self, Decoder, FromBencode, Object};

#[derive(Debug)]
pub struct TorrentBytes {
    pub info: TorrentBytesInfo,
    pub creation_date: Option<u32>,
}

#[derive(Debug)]
pub struct TorrentBytesInfo {
    pub name: String,
    pub source: Option<String>,
    pub file_length: Option<u64>,
    pub files: Option<Vec<TorrentBytesFile>>,
}

#[derive(Debug)]
pub struct TorrentBytesFile {
    pub path: Vec<String>,
    pub length: u64,
}

impl FromBencode for TorrentBytes {
    fn decode_bencode_object(object: Object) -> Result<Self, decoding::Error>
    where
        Self: Sized,
    {
        let mut creation_date = None;
        let mut info = None;

        let mut dict_dec = object.try_into_dictionary()?;
        while let Some(pair) = dict_dec.next_pair()? {
            match pair {
                (b"creation date", value) => {
                    creation_date = Some(u32::decode_bencode_object(value)?);
                }
                (b"info", value) => {
                    info = Some(TorrentBytesInfo::decode_bencode_object(value)?);
                }
                _ => {}
            }
        }

        let info = info.ok_or_else(|| decoding::Error::missing_field("info"))?;
        Ok(TorrentBytes {
            info,
            creation_date,
        })
    }
}

impl FromBencode for TorrentBytesInfo {
    fn decode_bencode_object(object: Object) -> Result<Self, decoding::Error>
    where
        Self: Sized,
    {
        let bytes = object.try_into_dictionary()?;
        let bytes = bytes.into_raw()?;
        let mut decoder = Decoder::new(bytes);
        let mut dict = decoder.next_object()?.unwrap().try_into_dictionary()?;

        let mut files = None;
        let mut file_length = None;
        let mut name = None;
        let mut source = None;

        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"source", value) => {
                    source = Some(String::decode_bencode_object(value)?);
                }
                (b"name", value) => {
                    name = Some(String::decode_bencode_object(value)?);
                }
                (b"files", value) => {
                    files = Some(Vec::<TorrentBytesFile>::decode_bencode_object(value)?)
                }
                (b"length", value) => {
                    file_length = Some(u64::decode_bencode_object(value)?);
                }
                _ => {}
            }
        }

        let name = name.ok_or_else(|| decoding::Error::missing_field("name"))?;
        Ok(TorrentBytesInfo {
            files,
            file_length,
            name,
            source,
        })
    }
}

impl FromBencode for TorrentBytesFile {
    fn decode_bencode_object(object: Object) -> Result<Self, decoding::Error>
    where
        Self: Sized,
    {
        let mut dict_dec = object.try_into_dictionary()?;
        let mut length = None;
        let mut path = None;

        while let Some(pair) = dict_dec.next_pair()? {
            match pair {
                (b"length", value) => {
                    length = Some(u64::decode_bencode_object(value)?);
                }
                (b"path", value) => {
                    path = Some(Vec::<String>::decode_bencode_object(value)?);
                }
                _ => {}
            }
        }

        let length = length.ok_or_else(|| decoding::Error::missing_field("length"))?;
        let path = path.ok_or_else(|| decoding::Error::missing_field("path"))?;

        Ok(Self { length, path })
    }
}
