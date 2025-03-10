pub trait FileStorage {
    fn save_file();
}

pub struct LocalFileStorage {}

impl FileStorage for LocalFileStorage {
    fn save_file() {
        todo!()
    }
}
