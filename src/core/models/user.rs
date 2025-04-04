/// Represents a user in the application domain
#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
}

impl User {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            display_name: None,
        }
    }
    
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }
    
    pub fn display_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }
}