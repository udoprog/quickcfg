/// Macro to populate a system struct with its base fields.
macro_rules! system_struct {
    (
        $(#[$name_meta:meta])*
        $name:ident {
        $(
        $(#[$attr:meta])*
        pub $field:ident : $field_ty:ty,
        )*
    }) => {
        $(#[$name_meta])*
        #[derive(::serde::Deserialize, Debug, PartialEq, Eq)]
        pub struct $name {
            /// Id of this system.
            pub id: Option<String>,

            #[serde(default)]
            /// Things that this system requires.
            pub requires: Vec<String>,

            $($(#[$attr])* pub $field: $field_ty,)*
        }

        impl $name {
            pub fn id(&self) -> Option<&str> {
                self.id.as_ref().map(|s| s.as_str())
            }

            pub fn requires(&self) -> &[String] {
                &self.requires
            }
        }
    }
}

macro_rules! system_defaults {
    (@method translate) => {
        /// Default translation implementation for the given system.
        pub fn translate(&self) -> crate::system::Translation<'_> {
            crate::system::Translation::Keep
        }
    };

    ($($name:ident),*) => {
        $(system_defaults!(@method $name);)*
    };
}
