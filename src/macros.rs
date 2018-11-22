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

            $(
            $(#[$attr])*
            pub $field: $field_ty,
            )*
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

macro_rules! system_functions {
    ($($name:ident,)*) => {
        /// Get the id of this system.
        pub fn id(&self) -> Option<&str> {
            use self::System::*;

            match *self {
                $($name(ref system) => system.id(),)*
            }
        }

        /// Get all things that this system depends on.
        pub fn requires(&self) -> &[String] {
            use self::System::*;

            match *self {
                $($name(ref system) => system.requires(),)*
            }
        }

        /// Apply changes for this system.
        pub fn apply<E>(&self, input: $crate::system::SystemInput<E>)
            -> Result<Vec<$crate::system::SystemUnit>, Error>
        where
            E: Copy + $crate::environment::Environment,
        {
            use self::System::*;

            match *self {
                $($name(ref system) => system.apply(input),)*
            }
        }
    }
}