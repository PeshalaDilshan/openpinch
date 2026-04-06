package connectors

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
)

type TelegramConnector struct {
	cfg          *config.Config
	token        string
	pollInterval time.Duration
	client       *http.Client
	bridge       *enginebridge.Client
}

func NewTelegram(cfg *config.Config, bridge *enginebridge.Client) Connector {
	return &TelegramConnector{
		cfg:          cfg,
		token:        cfg.Gateway.TelegramBotToken,
		pollInterval: time.Duration(cfg.Gateway.TelegramPollIntervalSeconds) * time.Second,
		client:       &http.Client{Timeout: 30 * time.Second},
		bridge:       bridge,
	}
}

func (t *TelegramConnector) Name() string {
	return "telegram"
}

func (t *TelegramConnector) Enabled() bool {
	return t.token != ""
}

func (t *TelegramConnector) Descriptor() Descriptor {
	connector := t.cfg.Connectors["telegram"]
	return Descriptor{
		Name:        "telegram",
		Enabled:     t.Enabled(),
		Implemented: true,
		Mode:        firstNonEmpty(connector.Mode, "polling"),
		Health:      healthFor(t.Enabled(), true),
		Allowlist:   connector.Allowlist,
		Details: map[string]string{
			"transport":             "telegram-bot-api",
			"poll_interval_seconds": fmt.Sprintf("%d", t.cfg.Gateway.TelegramPollIntervalSeconds),
		},
	}
}

func (t *TelegramConnector) Start(ctx context.Context) error {
	var offset int64
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		updates, err := t.getUpdates(ctx, offset)
		if err != nil {
			time.Sleep(t.pollInterval)
			continue
		}

		for _, update := range updates {
			offset = update.UpdateID + 1
			if update.Message == nil || update.Message.Text == "" {
				continue
			}

			reply, err := t.bridge.DeliverMessage(ctx, enginebridge.Message{
				Connector:    "telegram",
				ChannelID:    strconv.FormatInt(update.Message.Chat.ID, 10),
				Sender:       update.Message.From.Username,
				Body:         update.Message.Text,
				MetadataJSON: "{}",
			})
			if err != nil || reply == nil || reply.Reply == "" {
				continue
			}
			_ = t.sendMessage(ctx, update.Message.Chat.ID, reply.Reply)
		}

		time.Sleep(t.pollInterval)
	}
}

func (t *TelegramConnector) getUpdates(ctx context.Context, offset int64) ([]telegramUpdate, error) {
	requestURL := fmt.Sprintf(
		"https://api.telegram.org/bot%s/getUpdates?timeout=25&offset=%d",
		t.token,
		offset,
	)
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, requestURL, nil)
	if err != nil {
		return nil, err
	}

	response, err := t.client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	var payload telegramUpdatesResponse
	if err := json.NewDecoder(response.Body).Decode(&payload); err != nil {
		return nil, err
	}
	return payload.Result, nil
}

func (t *TelegramConnector) sendMessage(ctx context.Context, chatID int64, text string) error {
	form := url.Values{}
	form.Set("chat_id", strconv.FormatInt(chatID, 10))
	form.Set("text", text)

	request, err := http.NewRequestWithContext(
		ctx,
		http.MethodPost,
		fmt.Sprintf("https://api.telegram.org/bot%s/sendMessage", t.token),
		strings.NewReader(form.Encode()),
	)
	if err != nil {
		return err
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := t.client.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	return nil
}

type telegramUpdatesResponse struct {
	Result []telegramUpdate `json:"result"`
}

type telegramUpdate struct {
	UpdateID int64            `json:"update_id"`
	Message  *telegramMessage `json:"message"`
}

type telegramMessage struct {
	Text string       `json:"text"`
	Chat telegramChat `json:"chat"`
	From telegramUser `json:"from"`
}

type telegramChat struct {
	ID int64 `json:"id"`
}

type telegramUser struct {
	Username string `json:"username"`
}
