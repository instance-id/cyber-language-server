
#[macro_export]
macro_rules! position {
    ($line:expr, $character:expr) => {{
        Position::new($line as u32, $character as u32)
    }};
}

#[macro_export]
macro_rules! range_at {
    ($doc:expr, $sub:expr) => {{
        let index = $doc.text.find($sub).unwrap();
        lsp_types::Range::new(
            $doc.position_at(index as u32),
            $doc.position_at(index as u32 + $sub.len() as u32),
        )
    }};
}

#[macro_export]
macro_rules! range_after {
    ($doc:expr, $sub:expr ) => {{
        let index = $doc.text.find($sub).unwrap() + $sub.len();
        lsp_types::Range::new(
            $doc.position_at(index as u32),
            $doc.position_at(index as u32),
        )
    }};
}
#[macro_export]
/// an insert TextDocumentContentChangeEvent
macro_rules! ie {
    ($text:expr, $doc:expr, $sub_str:expr) => {
        {
            TextDocumentContentChangeEvent {
                text: $text.into(),
                range: Some(range_after!($doc, $sub_str)),
                range_length: None,
            }
        }
    };
}
#[macro_export]
macro_rules! re {
    ($text:expr, $doc:expr, $sub_str:expr) => {
        {
            TextDocumentContentChangeEvent {
                text: $text.into(),
                range: Some(range_at!($doc, $sub_str)),
                range_length: None,
            }
        }
    };
}

#[macro_export]
macro_rules! range {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        lsp_types::Range {
            start: position!($a, $b),
            end: position!($c, $d)
        }
    }

}

#[macro_export]
macro_rules! event {
    ($text:expr, $range:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            text: $text.to_string(),
            range: Some($range),
            range_length: None
        }
    }

}
