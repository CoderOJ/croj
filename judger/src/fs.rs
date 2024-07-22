use anyhow::{Error, Result};

pub struct File {
	name: String,
}

impl File {
	pub fn bind(name: &str) -> Self {
		Self {
			name: String::from(name),
		}
	}
	/// getter - open as read
	pub fn getter(&self) -> Result<std::fs::File> {
		std::fs::File::open(&self.name).map_err(|e| Error::from(e))
	}
	/// setter - open as write
	pub fn setter(&self) -> Result<std::fs::File> {
		std::fs::File::create(&self.name).map_err(|e| Error::from(e))
	}
	/// get - read to string
	pub fn get(&self) -> Result<String> {
		std::fs::read_to_string(&self.name).map_err(|e| Error::from(e))
	}
	/// set - write
	pub fn set<C: AsRef<[u8]>>(&self, contents: C) -> Result<()> {
		std::fs::write(&self.name, contents).map_err(|e| Error::from(e))
	}
	/// raw - filename
	pub fn raw(&self) -> &String {
		&self.name
	}
}

pub struct FileList {
	prefix: String,
}

pub struct FileListIter<'a> {
	parent: &'a FileList,
	id:     u64,
}

impl FileList {
	pub fn bind(prefix: &str) -> Self {
		Self {
			prefix: String::from(prefix),
		}
	}
	pub fn at(&self, uid: u64) -> File {
		File {
			name: format!("{}{}", self.prefix, uid),
		}
	}
	pub fn iter(&self) -> FileListIter {
		FileListIter {
			parent: self,
			id:     0,
		}
	}
}

impl<'a> Iterator for FileListIter<'a> {
	type Item = File;
	fn next(&mut self) -> Option<Self::Item> {
		self.id += 1;
		return Some(self.parent.at(self.id - 1));
	}
}

pub struct Fs {
	pub source:         File,
	pub target:         File,
	pub output:         File,
	pub compile_output: File,
	pub input:          FileList,
	pub answer:         FileList,
	pub checker:        FileList,
	pub checker_output: File,
}

impl Fs {
	// this function may only called once
	pub fn bind(dir: &str) -> Result<Self> {
		// ./a is the directory with permission 700
		std::env::set_current_dir(dir)?;
		return Ok(Fs {
			source:         File::bind("a/source"),
			target:         File::bind("target"),
			output:         File::bind("a/output"),
			compile_output: File::bind("a/compile_output"),
			input:          FileList::bind("a/data/in"),
			answer:         FileList::bind("a/data/ans"),
			checker:        FileList::bind("a/checker"),
			checker_output: File::bind("a/checker_output"),
		});
	}
}
