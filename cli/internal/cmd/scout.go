package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/notacamp/campbooks/cli/internal/client"
	"github.com/notacamp/campbooks/cli/internal/output"
	"github.com/spf13/cobra"
)

func newScoutCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "scout",
		Short: "Chat with Scout, the in-app AI assistant",
	}
	threads := &cobra.Command{Use: "threads", Short: "List and create Scout threads"}
	threads.AddCommand(scoutThreadsListCmd(), scoutThreadsNewCmd())
	cmd.AddCommand(threads, scoutAskCmd(), scoutMessagesCmd())
	return cmd
}

func scoutThreadsListCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List your Scout threads",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/scout/threads", nil)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				colTrunc("title", "title", 40),
				col("purpose", "purpose"),
				col("created", "created_at"),
			})
		},
	}
}

func scoutThreadsNewCmd() *cobra.Command {
	var title string
	cmd := &cobra.Command{
		Use:   "new",
		Short: "Create a Scout thread",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			form := url.Values{}
			setIf(form, "title", title)
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/scout/threads", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Created thread")
		},
	}
	cmd.Flags().StringVar(&title, "title", "", "thread title")
	return cmd
}

func scoutMessagesCmd() *cobra.Command {
	var after string
	cmd := &cobra.Command{
		Use:   "messages <thread-id>",
		Short: "Show a thread's messages",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			q := url.Values{}
			if after != "" {
				q.Set("after_message_id", after)
			}
			data, _, err := s.client.Get(cmd.Context(), "/api/v1/scout/threads/"+args[0]+"/messages", q)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, nil, []column{
				col("id", "id"),
				col("author", "author_type"),
				col("status", "reply_status"),
				colTrunc("content", "content", 70),
			})
		},
	}
	cmd.Flags().StringVar(&after, "after", "", "only messages after this message ID")
	return cmd
}

func scoutAskCmd() *cobra.Command {
	var (
		thread  string
		wait    bool
		timeout int
	)
	cmd := &cobra.Command{
		Use:   "ask <message>",
		Short: "Send a message to Scout (async; --wait prints the reply)",
		Args:  cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			ctx := cmd.Context()
			out := cmd.OutOrStdout()
			message := strings.Join(args, " ")

			threadID := thread
			if threadID == "" {
				data, err := s.client.Send(ctx, http.MethodPost, "/api/v1/scout/threads", nil, url.Values{})
				if err != nil {
					return err
				}
				var t map[string]any
				if err := json.Unmarshal(data, &t); err != nil {
					return err
				}
				threadID = stringField(t, "id")
			}

			data, err := s.client.Send(ctx, http.MethodPost,
				"/api/v1/scout/threads/"+threadID+"/messages", nil,
				url.Values{"content": {message}})
			if err != nil {
				return err
			}
			var userMsg map[string]any
			if err := json.Unmarshal(data, &userMsg); err != nil {
				return err
			}
			userMsgID := stringField(userMsg, "id")

			if !wait {
				fmt.Fprintf(out, "✓ Sent to thread %s (message %s). Read the reply with:\n  campbooks scout messages %s --after %s\n",
					threadID, userMsgID, threadID, userMsgID)
				return nil
			}

			fmt.Fprintln(cmd.ErrOrStderr(), "Waiting for Scout to reply…")
			reply, err := pollForReply(ctx, s.client, threadID, userMsgID, time.Duration(timeout)*time.Second)
			if err != nil {
				return err
			}
			if flagJSON {
				return output.JSON(out, reply)
			}
			fmt.Fprintln(out, cell(reply["content"]))
			return nil
		},
	}
	cmd.Flags().StringVar(&thread, "thread", "", "existing thread ID (default: start a new thread)")
	cmd.Flags().BoolVar(&wait, "wait", false, "wait for and print Scout's reply")
	cmd.Flags().IntVar(&timeout, "timeout", 120, "seconds to wait for the reply (with --wait)")
	return cmd
}

func pollForReply(ctx context.Context, c *client.Client, threadID, afterID string, timeout time.Duration) (map[string]any, error) {
	deadline := time.Now().Add(timeout)
	for {
		data, _, err := c.Get(ctx, "/api/v1/scout/threads/"+threadID+"/messages",
			url.Values{"after_message_id": {afterID}})
		if err != nil {
			return nil, err
		}
		var msgs []map[string]any
		if err := json.Unmarshal(data, &msgs); err != nil {
			return nil, err
		}
		for _, m := range msgs {
			if cell(m["author_type"]) != "ai" {
				continue
			}
			switch cell(m["reply_status"]) {
			case "replied":
				return m, nil
			case "failed":
				return nil, fmt.Errorf("Scout failed to generate a reply")
			}
		}
		if time.Now().After(deadline) {
			return nil, fmt.Errorf("timed out waiting for Scout (try `campbooks scout messages %s`)", threadID)
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(2 * time.Second):
		}
	}
}
