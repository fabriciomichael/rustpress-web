use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{
        Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::OsRng,
    },
};
use std::fmt;

/// Configuration used by the Argon2id algorithm.
#[derive(Debug, Clone, Copy)]
pub struct PasswordConfig {
    /// Amount of memory used by Argon2, in KiB.
    pub memory_cost: u32,

    /// Number of iterations performed by the algorithm.
    pub time_cost: u32,

    /// Number of lanes used in parallel.
    pub parallelism: u32,

    /// Size of the generated hash, in bytes.
    ///
    /// Cannot exceed 64 bytes.
    pub output_length: usize,
}

impl Default for PasswordConfig {
    fn default() -> Self {
        Self {
            memory_cost: 19_456,
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        }
    }
}

/// Errors that may occur while generating or verifying passwords.
#[derive(Debug)]
pub enum PasswordError {
    /// Failure to create the Argon2 parameters.
    Parameters(argon2::Error),

    /// Failure to generate the hash.
    Hashing(PasswordHashError),

    /// The provided hash does not have a valid format.
    InvalidHash(PasswordHashError),

    /// Failure during hash verification.
    Verification(PasswordHashError),

    /// The password exceeds the maximum allowed size.
    PasswordTooLong,

    /// The hash algorithm is not Argon2id.
    UnsupportedAlgorithm,

    /// The Argon2 version is not supported.
    UnsupportedVersion,

    /// The hash parameters exceed the allowed limits.
    UnsupportedHashParameters,

    /// The hash exceeds the maximum allowed size.
    HashTooLong,

    /// The configured output size exceeds the allowed limit.
    OutputLengthTooLarge,
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters(error) => {
                write!(f, "Parâmetros Argon2 inválidos: {error}")
            }
            Self::Hashing(error) => {
                write!(f, "Falha ao gerar o hash da palavra-passe: {error}")
            }
            Self::InvalidHash(error) => {
                write!(f, "Hash de palavra-passe inválido: {error}")
            }
            Self::Verification(error) => {
                write!(f, "Falha ao verificar a palavra-passe: {error}")
            }
            Self::PasswordTooLong => {
                write!(f, "A palavra-passe excede o limite de 1024 bytes")
            }
            Self::UnsupportedAlgorithm => {
                write!(f, "O algoritmo do hash não é Argon2id")
            }
            Self::UnsupportedVersion => {
                write!(f, "Versão do Argon2 não suportada")
            }
            Self::UnsupportedHashParameters => {
                write!(f, "Os parâmetros do hash excedem os limites permitidos")
            }
            Self::HashTooLong => {
                write!(f, "O hash excede o limite de 512 bytes")
            }
            Self::OutputLengthTooLarge => {
                write!(f, "O tamanho de saída excede o limite de 64 bytes")
            }
        }
    }
}

impl std::error::Error for PasswordError {}

/// Tools for generating and verifying password hashes.
pub struct Password;

impl Password {
    const MAX_PASSWORD_LENGTH: usize = 1024;

    const MAX_HASH_LENGTH: usize = 512;
    const MAX_OUTPUT_LENGTH: usize = 64;

    const MAX_MEMORY_COST: u32 = 262_144;
    const MAX_TIME_COST: u32 = 10;
    const MAX_PARALLELISM: u32 = 8;

    /// Generates an Argon2id hash with the default configuration.
    ///
    /// The hash includes the parameters required for verification.
    pub fn hash(password: &str) -> Result<String, PasswordError> {
        Self::hash_with_config(password, PasswordConfig::default())
    }

    /// Generates an Argon2id hash using a specific configuration.
    ///
    /// The output size cannot exceed 64 bytes.
    /// The parameters must respect RustPress resource limits
    /// and the restrictions imposed by Argon2.
    ///
    /// # Errors
    ///
    /// Returns [`PasswordError::OutputLengthTooLarge`] when the output
    /// size exceeds 64 bytes.
    /// Returns [`PasswordError::UnsupportedHashParameters`] when the
    /// parameters exceed the limits defined by RustPress.
    /// Returns [`PasswordError::Parameters`] when the parameters are
    /// invalid according to Argon2 rules.
    /// Returns [`PasswordError::PasswordTooLong`] when the password
    /// exceeds the maximum allowed length.
    pub fn hash_with_config(
        password: &str,
        config: PasswordConfig,
    ) -> Result<String, PasswordError> {
        Self::validate_password(password)?;

        if config.output_length > Self::MAX_OUTPUT_LENGTH {
            return Err(PasswordError::OutputLengthTooLarge);
        }

        if config.memory_cost == 0
            || config.time_cost == 0
            || config.parallelism == 0
            || config.memory_cost > Self::MAX_MEMORY_COST
            || config.time_cost > Self::MAX_TIME_COST
            || config.parallelism > Self::MAX_PARALLELISM
        {
            return Err(PasswordError::UnsupportedHashParameters);
        }

        let params = Params::new(
            config.memory_cost,
            config.time_cost,
            config.parallelism,
            Some(config.output_length),
        )
        .map_err(PasswordError::Parameters)?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let salt = SaltString::generate(&mut OsRng);

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(PasswordError::Hashing)
    }

    /// Verifies whether a password matches the provided hash.
    ///
    /// Accepts only Argon2id hashes in version 19,
    /// while respecting the resource limits defined by RustPress.
    ///
    /// Returns `Ok(true)` when it matches and `Ok(false)` when it does not.
    /// Invalid hashes and processing errors are returned as `Err`.
    pub fn verify(password: &str, hash: &str) -> Result<bool, PasswordError> {
        Self::validate_password(password)?;

        if hash.len() > Self::MAX_HASH_LENGTH {
            return Err(PasswordError::HashTooLong);
        }

        let parsed_hash = PasswordHash::new(hash).map_err(PasswordError::InvalidHash)?;

        if parsed_hash.algorithm.as_str() != "argon2id" {
            return Err(PasswordError::UnsupportedAlgorithm);
        }

        if parsed_hash.version != Some(19) {
            return Err(PasswordError::UnsupportedVersion);
        }

        let (Some(memory_cost), Some(time_cost), Some(parallelism)) = (
            parsed_hash.params.get_decimal("m"),
            parsed_hash.params.get_decimal("t"),
            parsed_hash.params.get_decimal("p"),
        ) else {
            return Err(PasswordError::UnsupportedHashParameters);
        };

        if memory_cost == 0
            || time_cost == 0
            || parallelism == 0
            || memory_cost > Self::MAX_MEMORY_COST
            || time_cost > Self::MAX_TIME_COST
            || parallelism > Self::MAX_PARALLELISM
        {
            return Err(PasswordError::UnsupportedHashParameters);
        }

        match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(PasswordHashError::Password) => Ok(false),
            Err(error) => Err(PasswordError::Verification(error)),
        }
    }

    fn validate_password(password: &str) -> Result<(), PasswordError> {
        if password.len() > Self::MAX_PASSWORD_LENGTH {
            return Err(PasswordError::PasswordTooLong);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Password;
    use crate::security::password::{PasswordConfig, PasswordError};

    #[test]
    fn generates_argon2id_hash() {
        let hash = Password::hash("minha_senha").unwrap();

        assert!(hash.starts_with("$argon2id$"));
        assert_ne!(hash, "minha_senha");
    }

    #[test]
    fn verifies_correct_password() {
        let hash = Password::hash("minha_senha").unwrap();

        assert!(Password::verify("minha_senha", &hash).unwrap());
    }

    #[test]
    fn rejects_incorrect_password() {
        let hash = Password::hash("minha_senha").unwrap();

        assert!(!Password::verify("senha_errada", &hash).unwrap());
    }

    #[test]
    fn generates_different_hashes_for_same_password() {
        let first = Password::hash("minha_senha").unwrap();
        let second = Password::hash("minha_senha").unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn rejects_invalid_hash() {
        let result = Password::verify("minha_senha", "hash_invalido");

        assert!(result.is_err());
    }

    #[test]
    fn custom_config_generates_hash_with_configured_parameters() {
        let config = PasswordConfig {
            memory_cost: 8_192,
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        let hash = Password::hash_with_config("minha_senha", config).unwrap();

        assert!(hash.starts_with("$argon2id$"));
        assert!(hash.contains("m=8192,t=2,p=1"));
    }

    #[test]
    fn custom_config_hash_can_be_verified() {
        let config = PasswordConfig {
            memory_cost: 8_192,
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        let hash = Password::hash_with_config("minha_senha", config).unwrap();

        assert!(Password::verify("minha_senha", &hash).unwrap());
        assert!(!Password::verify("senha_errada", &hash).unwrap());
    }

    #[test]
    fn rejects_excessive_output_length() {
        let config = PasswordConfig {
            memory_cost: 19_456,
            time_cost: 2,
            parallelism: 1,
            output_length: 65,
        };

        let result = Password::hash_with_config("minha_senha", config);

        assert!(matches!(result, Err(PasswordError::OutputLengthTooLarge)));
    }

    #[test]
    fn rejects_invalid_password_config() {
        let config = PasswordConfig {
            memory_cost: 0,
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        let result = Password::hash_with_config("minha_senha", config);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_memory_cost_when_hashing() {
        let config = PasswordConfig {
            memory_cost: 262_145,
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        let result = Password::hash_with_config("minha_senha", config);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_time_cost_when_hashing() {
        let config = PasswordConfig {
            memory_cost: 19_456,
            time_cost: 11,
            parallelism: 1,
            output_length: 32,
        };

        let result = Password::hash_with_config("minha_senha", config);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_parallelism_when_hashing() {
        let config = PasswordConfig {
            memory_cost: 19_456,
            time_cost: 2,
            parallelism: 9,
            output_length: 32,
        };

        let result = Password::hash_with_config("minha_senha", config);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_password_too_long_when_hashing() {
        let password = "a".repeat(1025);

        let result = Password::hash(&password);

        assert!(matches!(result, Err(PasswordError::PasswordTooLong)));
    }

    #[test]
    fn rejects_password_too_long_when_verifying() {
        let password = "a".repeat(1025);

        let result = Password::verify(&password, "hash_invalido");

        assert!(matches!(result, Err(PasswordError::PasswordTooLong)));
    }

    #[test]
    fn accepts_password_at_maximum_length() {
        let password = "a".repeat(1024);

        let hash = Password::hash(&password).unwrap();

        assert!(Password::verify(&password, &hash).unwrap());
    }

    #[test]
    fn validates_unicode_password_by_byte_length() {
        let valid_password = "é".repeat(512);
        let invalid_password = "é".repeat(513);

        assert_eq!(valid_password.len(), 1024);
        assert_eq!(invalid_password.len(), 1026);

        assert!(Password::hash(&valid_password).is_ok());

        assert!(matches!(
            Password::hash(&invalid_password),
            Err(PasswordError::PasswordTooLong)
        ));
    }

    #[test]
    fn rejects_non_argon2id_algorithm() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("$argon2id$", "$argon2d$");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(result, Err(PasswordError::UnsupportedAlgorithm)));
    }

    #[test]
    fn rejects_unsupported_argon2_version() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("$v=19$", "$v=16$");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(result, Err(PasswordError::UnsupportedVersion)));
    }

    #[test]
    fn rejects_zero_memory_cost() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("m=19456", "m=0");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_zero_time_cost() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("t=2", "t=0");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_zero_parallelism() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("p=1", "p=0");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_memory_cost() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("m=19456", "m=262145");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_time_cost() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("t=2", "t=11");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessive_parallelism() {
        let hash = Password::hash("minha_senha").unwrap();
        let modified_hash = hash.replace("p=1", "p=9");

        let result = Password::verify("minha_senha", &modified_hash);

        assert!(matches!(
            result,
            Err(PasswordError::UnsupportedHashParameters)
        ));
    }

    #[test]
    fn rejects_excessively_long_hash() {
        let hash = "x".repeat(513);

        let result = Password::verify("minha_senha", &hash);

        assert!(matches!(result, Err(PasswordError::HashTooLong)));
    }
}
