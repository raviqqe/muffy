pub const TEXT_TOKEN: &str = "#text";

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Content {
    Choice(&'static [Self]),
    Element(&'static [&'static str]),
    Empty,
    Group(&'static [Self]),
    Interleave(&'static [Self]),
    Many0(&'static Self),
    Many1(&'static Self),
    Optional(&'static Self),
    Text,
}

impl Content {
    pub fn nullable(&self) -> bool {
        match self {
            Self::Choice(patterns) => patterns.iter().any(Self::nullable),
            Self::Element(_) => false,
            Self::Empty | Self::Many0(_) | Self::Optional(_) | Self::Text => true,
            Self::Group(patterns) | Self::Interleave(patterns) => {
                patterns.iter().all(Self::nullable)
            }
            Self::Many1(pattern) => pattern.nullable(),
        }
    }
}
