package adapters

import (
	"context"
	"io"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

// ReadObject returns private object bytes directly from S3. Workers use this
// instead of following presigned URLs so approved attachments never leave the
// trusted server path before SMTP transport.
func (s *S3) ReadObject(ctx context.Context, key string, max int64) ([]byte, error) {
	if s == nil || s.Client == nil || key == "" || max < 1 || max > core.MaxEmailAttachmentBytes {
		return nil, core.Invalid
	}
	r, e := s.Client.GetObject(ctx, &s3.GetObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key)})
	if e != nil {
		return nil, e
	}
	defer r.Body.Close()
	b, e := io.ReadAll(io.LimitReader(r.Body, max+1))
	if e != nil {
		return nil, e
	}
	if int64(len(b)) > max {
		return nil, core.Invalid
	}
	return b, nil
}
