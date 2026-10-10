package adapters

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
	"mime"
	"net/url"
	"testing"
)

func TestPreviewPresignHasSafeSignedTypeAndInlineFilename(t *testing.T) {
	c := s3.NewFromConfig(aws.Config{Region: "us-east-1", Credentials: aws.CredentialsProviderFunc(func(context.Context) (aws.Credentials, error) {
		return aws.Credentials{AccessKeyID: "synthetic", SecretAccessKey: "synthetic-test-only"}, nil
	})})
	store := S3{Client: c, Presign: s3.NewPresignClient(c), Bucket: "synthetic-private-test"}
	for _, media := range []string{"application/pdf", "image/png", "image/jpeg", "text/plain"} {
		raw, e := store.PreviewURL(context.Background(), "private/test", "nombre con acentos á.pdf", media)
		if e != nil {
			t.Fatal(e)
		}
		u, e := url.Parse(raw)
		if e != nil {
			t.Fatal(e)
		}
		q := u.Query()
		actual := media
		if media == "text/plain" {
			actual += "; charset=utf-8"
		}
		disposition, params, e := mime.ParseMediaType(q.Get("response-content-disposition"))
		if e != nil || disposition != "inline" || params["filename"] != "nombre con acentos á.pdf" || q.Get("response-content-type") != actual || q.Get("X-Amz-Expires") != "300" || q.Get("X-Amz-Signature") == "" {
			t.Fatal("unsafe presign parameters", e)
		}
	}
	for _, media := range []string{"text/html", "image/svg+xml", "application/octet-stream"} {
		if _, e := store.PreviewURL(context.Background(), "private/test", "test", media); e != core.Invalid {
			t.Fatal("active content accepted", e)
		}
	}
}
