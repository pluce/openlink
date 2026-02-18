//! Error types for the `openlink-models` crate.
//!
//! All fallible constructors and `TryFrom` implementations in this crate
//! return variants of [`ModelError`].

/// Errors produced when constructing or validating model types.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// An ICAO airport code was not exactly 4 uppercase ASCII letters.
    #[error("invalid ICAO airport code \"{value}\": {reason}")]
    InvalidICAOCode {
        /// The value that failed validation.
        value: String,
        /// Human-readable explanation.
        reason: String,
    },

    /// An ACARS callsign was empty or contained invalid characters.
    #[error("invalid ACARS callsign \"{value}\": {reason}")]
    InvalidCallsign {
        /// The value that failed validation.
        value: String,
        /// Human-readable explanation.
        reason: String,
    },

    /// A flight level string was not in the expected format.
    #[error("invalid flight level \"{value}\": {reason}")]
    InvalidFlightLevel {
        /// The value that failed validation.
        value: String,
        /// Human-readable explanation.
        reason: String,
    },

    /// A required field was missing during message construction.
    #[error("missing required field: {field}")]
    MissingField {
        /// The name of the missing field.
        field: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_icao() {
        let err = ModelError::InvalidICAOCode {
            value: "LF".into(),
            reason: "must be exactly 4 uppercase ASCII letters".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid ICAO airport code \"LF\": must be exactly 4 uppercase ASCII letters"
        );
    }

    #[test]
    fn error_display_callsign() {
        let err = ModelError::InvalidCallsign {
            value: "".into(),
            reason: "must not be empty".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid ACARS callsign \"\": must not be empty"
        );
    }

    #[test]
    fn error_display_flight_level() {
        let err = ModelError::InvalidFlightLevel {
            value: "abc".into(),
            reason: "must be a number between 0 and 999".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid flight level \"abc\": must be a number between 0 and 999"
        );
    }

    #[test]
    fn error_display_missing_field() {
        let err = ModelError::MissingField {
            field: "source".into(),
        };
        assert_eq!(err.to_string(), "missing required field: source");
    }
}
