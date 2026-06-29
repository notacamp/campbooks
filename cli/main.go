// Command campbooks is the developer CLI for Campbooks — Not A Camp's AI-native
// email client. It wraps the public REST API: sign in with `campbooks login`,
// then read/send email, manage documents, contacts and tags, and talk to Scout.
package main

import "github.com/notacamp/campbooks/cli/internal/cmd"

// version is overridden at build time via -ldflags "-X main.version=vX.Y.Z".
var version = "dev"

func main() {
	cmd.Execute(version)
}
