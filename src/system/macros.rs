/// Macro to populate a system struct with its base fields.
macro_rules! system_struct {
    ($name:ident {
        $(
        $(#[$attr:meta])*
        pub $field:ident : $field_ty:ty,
        )*
    }) => {
        #[derive(Deserialize, Debug, PartialEq, Eq)]
        pub struct $name {
            /// Id of this system.
            pub id: Option<String>,

            #[serde(default)]
            /// Things that this system requires.
            pub requires: Vec<String>,

            $($(#[$attr])* pub $field: $field_ty,)*
        }

        impl $name {
            pub fn id<'a>(&'a self) -> Option<&'a str> {
                self.id.as_ref().map(|s| s.as_str())
            }

            pub fn requires<'a>(&'a self) -> &[String] {
                &self.requires
            }
        }
    }
}
