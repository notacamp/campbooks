package auth

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
)

// newVerifier returns a high-entropy PKCE code verifier (RFC 7636): 64 random
// bytes → 86 url-safe chars, within the required 43–128 range.
func newVerifier() (string, error) {
	b := make([]byte, 64)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

// challengeS256 is the S256 code challenge for a verifier.
func challengeS256(verifier string) string {
	sum := sha256.Sum256([]byte(verifier))
	return base64.RawURLEncoding.EncodeToString(sum[:])
}

// randomState returns an opaque CSRF state value for the authorize request.
func randomState() (string, error) {
	b := make([]byte, 24)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}
