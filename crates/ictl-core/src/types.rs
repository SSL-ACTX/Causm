use crate::{BuiltinType, TypeName};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructType {
    pub fields: HashMap<String, Type>,
    pub decay_after_ms: Option<u64>,
    pub scoped_branch: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Integer,
    Float,
    Bool,
    String,
    Struct(StructType),
    Topology(HashMap<String, Type>),
    Array(Box<Type>),
    Optional(Box<Type>),
    Union(Vec<Type>),
    Generic(String),
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    PacedIterable {
        element_type: Box<Type>,
        max_time_ms: u64,
    },
    ConstantAccess {
        inner_type: Box<Type>,
        access_time_ms: u64,
    },
    Custom(String),
    Unknown,
}

impl Type {
    pub fn from_typename(type_name: &TypeName) -> Type {
        match type_name {
            TypeName::Builtin(b) => match b {
                BuiltinType::Integer => Type::Integer,
                BuiltinType::Float => Type::Float,
                BuiltinType::Bool => Type::Bool,
                BuiltinType::String => Type::String,
                BuiltinType::Struct => Type::Struct(StructType {
                    fields: HashMap::new(),
                    decay_after_ms: None,
                    scoped_branch: None,
                }),
                BuiltinType::Topology => Type::Topology(HashMap::new()),
                BuiltinType::Array => Type::Array(Box::new(Type::Unknown)),
            },
            TypeName::Custom(name) => Type::Custom(name.clone()),
            TypeName::Generic(name, params) => match name.as_str() {
                "PacedIterable" => {
                    let element_type =
                        if let Some(crate::TypeParam::Type(t)) = params.get(0) {
                            Box::new(Type::from_typename(t))
                        } else {
                            Box::new(Type::Unknown)
                        };
                    let max_time_ms = if let Some(crate::TypeParam::Duration(d)) =
                        params.get(1)
                    {
                        *d
                    } else if let Some(crate::TypeParam::Amount(a)) = params.get(1) {
                        *a
                    } else {
                        0
                    };
                    Type::PacedIterable {
                        element_type,
                        max_time_ms,
                    }
                }
                "ConstantAccess" => {
                    let inner_type =
                        if let Some(crate::TypeParam::Type(t)) = params.get(0) {
                            Box::new(Type::from_typename(t))
                        } else {
                            Box::new(Type::Unknown)
                        };
                    let access_time_ms = if let Some(crate::TypeParam::Duration(d)) =
                        params.get(1)
                    {
                        *d
                    } else if let Some(crate::TypeParam::Amount(a)) = params.get(1) {
                        *a
                    } else {
                        0
                    };
                    Type::ConstantAccess {
                        inner_type,
                        access_time_ms,
                    }
                }
                _ => Type::Custom(name.clone()),
            },
            TypeName::Optional(inner) => {
                Type::Optional(Box::new(Type::from_typename(inner)))
            }
            TypeName::Union(parts) => {
                Type::Union(parts.iter().map(Type::from_typename).collect())
            }
        }
    }

    #[allow(unused)]
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Integer | Type::Float)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Integer)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float)
    }

    #[allow(unused)]
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }

    #[allow(unused)]
    pub fn is_string(&self) -> bool {
        matches!(self, Type::String)
    }
}
