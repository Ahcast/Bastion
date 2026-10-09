package config

import (
	"encoding/base64"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/go-sql-driver/mysql"
	"net"
	"net/url"
	"os"
	"strings"
	"time"
)

type Config struct {
	Env, Listen, Origin, DSN, Bucket, Region string
	Key                                      []byte
	MCP, Email, Files                        bool
	SMTPHosts                                []string
	Idle, Absolute, Reauth                   time.Duration
}

func Load() (Config, error) {
	c := Config{Env: env("BASTION_ENV", "development"), Listen: env("BASTION_LISTEN", "127.0.0.1:8080"), Origin: env("BASTION_FRONTEND_ORIGIN", "http://localhost:4200"), DSN: os.Getenv("BASTION_MYSQL_DSN"), Bucket: os.Getenv("BASTION_S3_BUCKET"), Region: env("BASTION_S3_REGION", "us-east-1"), MCP: os.Getenv("BASTION_MCP_ENABLED") == "true", Email: os.Getenv("BASTION_EMAIL_ENABLED") == "true", Files: os.Getenv("BASTION_FILES_ENABLED") == "true", SMTPHosts: strings.FieldsFunc(os.Getenv("BASTION_SMTP_ALLOWED_HOSTS"), func(r rune) bool { return r == ',' }), Idle: 30 * time.Minute, Absolute: 12 * time.Hour, Reauth: 5 * time.Minute}
	var err error
	c.Key, err = base64.StdEncoding.DecodeString(os.Getenv("BASTION_ENCRYPTION_KEY"))
	if err != nil || len(c.Key) != 32 {
		return c, fmt.Errorf("BASTION_ENCRYPTION_KEY must be base64 32 bytes")
	}
	u, err := url.Parse(c.Origin)
	if err != nil || u.Host == "" || u.Path != "" || u.RawQuery != "" || u.Fragment != "" || u.User != nil {
		return c, fmt.Errorf("invalid frontend origin")
	}
	d, err := mysql.ParseDSN(c.DSN)
	if err != nil {
		return c, err
	}
	if d.Timeout == 0 {
		d.Timeout = 5 * time.Second
	}
	if d.ReadTimeout == 0 {
		d.ReadTimeout = 10 * time.Second
	}
	if d.WriteTimeout == 0 {
		d.WriteTimeout = 10 * time.Second
	}
	c.DSN = d.FormatDSN()
	if !d.ParseTime {
		return c, fmt.Errorf("DSN requires parseTime=true")
	}
	if d.DBName == "" || d.Loc.String() != "UTC" {
		return c, fmt.Errorf("DSN requires database and UTC loc")
	}
	if c.Env != "production" && c.Env != "development" {
		return c, fmt.Errorf("BASTION_ENV must be development or production")
	}
	if c.Env == "development" {
		host, _, err := net.SplitHostPort(c.Listen)
		if err != nil || (host != "127.0.0.1" && host != "localhost" && host != "::1") {
			return c, fmt.Errorf("development listen must be loopback")
		}
		if u.Hostname() != "localhost" && u.Hostname() != "127.0.0.1" && u.Hostname() != "::1" {
			return c, fmt.Errorf("development frontend must be loopback")
		}
	}
	denylist := os.Getenv("BASTION_PASSWORD_DENYLIST_FILE")
	if c.Env == "production" && denylist == "" {
		return c, fmt.Errorf("production requires a curated password denylist")
	}
	if denylist != "" {
		if e := security.LoadDenylist(denylist); e != nil {
			return c, e
		}
	}
	if c.Env == "production" {
		if u.Scheme != "https" {
			return c, fmt.Errorf("production requires HTTPS")
		}
		if d.TLSConfig == "" || d.TLSConfig == "false" || d.TLSConfig == "skip-verify" || d.TLSConfig == "preferred" {
			if os.Getenv("BASTION_DB_TUNNEL_VERIFIED") != "true" || !(strings.HasPrefix(d.Addr, "127.0.0.1:") || strings.HasPrefix(d.Addr, "localhost:")) {
				return c, fmt.Errorf("production requires verified MySQL TLS or verified local tunnel")
			}
		}
	} else if u.Scheme != "http" && u.Scheme != "https" {
		return c, fmt.Errorf("invalid origin scheme")
	}
	if c.Files && c.Bucket == "" {
		return c, fmt.Errorf("files require bucket")
	}
	return c, nil
}
func env(k, d string) string {
	if s := os.Getenv(k); s != "" {
		return s
	}
	return d
}
