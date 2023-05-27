#[derive(Debug, Default)]
pub struct EscapedEntry {
    pub key: String,
    pub data: String,
}

impl EscapedEntry {
    pub fn clear(&mut self) {
        self.key.clear();
        self.data.clear();
    }

    pub fn decoded_key(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.key)
    }

    pub fn decoded_data(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.data)
    }
}
