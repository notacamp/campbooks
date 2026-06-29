package cmd

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/notacamp/campbooks/cli/internal/client"
	"github.com/notacamp/campbooks/cli/internal/config"
)

func TestPollForReply(t *testing.T) {
	var calls int
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		calls++
		w.Header().Set("Content-Type", "application/json")
		if calls < 2 {
			// AI reply not ready yet.
			_, _ = io.WriteString(w, `{"data":[]}`)
			return
		}
		_, _ = io.WriteString(w, `{"data":[{"id":101,"author_type":"ai","reply_status":"replied","content":"Here you go."}]}`)
	}))
	defer srv.Close()

	c := client.New(srv.URL, &config.Auth{AccessToken: "t"}, nil)
	reply, err := pollForReply(context.Background(), c, "7", "100", 5*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	if cell(reply["content"]) != "Here you go." {
		t.Errorf("reply = %v", reply)
	}
}

func TestPollForReplyFailed(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"data":[{"id":1,"author_type":"ai","reply_status":"failed"}]}`)
	}))
	defer srv.Close()

	c := client.New(srv.URL, &config.Auth{AccessToken: "t"}, nil)
	if _, err := pollForReply(context.Background(), c, "7", "100", 5*time.Second); err == nil {
		t.Fatal("expected a failure error")
	}
}

func TestPollForReplyTimeout(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"data":[]}`) // never any AI reply
	}))
	defer srv.Close()

	c := client.New(srv.URL, &config.Auth{AccessToken: "t"}, nil)
	// Zero timeout → the deadline passes on the first empty poll.
	if _, err := pollForReply(context.Background(), c, "7", "100", 0); err == nil {
		t.Fatal("expected a timeout error")
	}
}
