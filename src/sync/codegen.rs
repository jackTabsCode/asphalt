use std::{collections::HashMap, path::PathBuf};

pub type CodegenInput = HashMap<PathBuf, HashMap<PathBuf, String>>;
