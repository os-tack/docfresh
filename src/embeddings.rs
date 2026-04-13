use std::collections::HashMap;
use std::process::Command;

pub struct EmbeddingCache {
    cache: HashMap<String, Vec<f32>>,
    available: Option<bool>, // None = not checked yet
}

impl EmbeddingCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            available: None,
        }
    }

    /// Embed text by shelling out to `ostk embeddings embed --text <text> --json`.
    /// Caches results for the lifetime of this cache instance.
    /// Returns Err if ostk is not on PATH or embeddings subcommand fails.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        if let Some(cached) = self.cache.get(text) {
            return Ok(cached.clone());
        }

        if !self.is_available() {
            return Err("ostk embeddings not available".to_string());
        }

        let output = Command::new("ostk")
            .args(["embeddings", "embed", "--text", text, "--json"])
            .output()
            .map_err(|e| format!("failed to run ostk: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ostk embeddings failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("failed to parse JSON: {e}"))?;

        #[allow(clippy::cast_possible_truncation)]
        let embedding: Vec<f32> = parsed
            .get("embedding")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "missing 'embedding' array in JSON output".to_string())?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        if embedding.is_empty() {
            return Err("empty embedding vector".to_string());
        }

        self.cache.insert(text.to_string(), embedding.clone());
        Ok(embedding)
    }

    /// Check if ostk embeddings is available (lazy, cached).
    pub fn is_available(&mut self) -> bool {
        if let Some(avail) = self.available {
            return avail;
        }

        let avail = Self::check_available();
        self.available = Some(avail);
        avail
    }

    fn check_available() -> bool {
        // Check if ostk exists
        let version_ok = Command::new("ostk")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !version_ok {
            return false;
        }

        // Verify embeddings subcommand exists
        Command::new("ostk")
            .args(["embeddings", "--help"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Cosine similarity between two vectors.
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() || a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((EmbeddingCache::similarity(&a, &a) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(EmbeddingCache::similarity(&a, &b).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_parallel() {
        let a = vec![1.0, 1.0];
        let b = vec![1.0, 1.0];
        assert!((EmbeddingCache::similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert!((EmbeddingCache::similarity(&a, &b)).abs() < f32::EPSILON);
    }

    #[test]
    fn cache_new_does_not_panic() {
        let _cache = EmbeddingCache::new();
    }

    #[test]
    fn similarity_antiparallel() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((EmbeddingCache::similarity(&a, &b) - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_different_lengths() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!((EmbeddingCache::similarity(&a, &b)).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((EmbeddingCache::similarity(&a, &b)).abs() < f32::EPSILON);
    }

    #[test]
    fn graceful_degradation_when_ostk_unavailable() {
        // In test environments, ostk is typically not installed.
        // Verify that embed() returns an error rather than panicking.
        let mut cache = EmbeddingCache::new();
        let result = cache.embed("test text");
        // Either it works (ostk installed) or it fails gracefully
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(
                err.contains("not available") || err.contains("failed"),
                "unexpected error: {err}"
            );
        }
    }
}
