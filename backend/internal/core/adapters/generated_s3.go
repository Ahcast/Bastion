package adapters

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
	"github.com/aws/smithy-go"
)

// PutImmutable never replaces a confirmed derived artifact. A retry verifies
// the existing object's SHA256 and length instead of overwriting its bytes.
func (s *S3) PutImmutable(ctx context.Context, key string, data []byte, hash string) error {
	raw, e := hex.DecodeString(hash)
	if e != nil || len(raw) != 32 {
		return core.Invalid
	}
	checksum := base64.StdEncoding.EncodeToString(raw)
	_, e = s.Client.PutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key), Body: bytes.NewReader(data), ContentType: aws.String("application/pdf"), ContentLength: aws.Int64(int64(len(data))), ChecksumSHA256: aws.String(checksum), IfNoneMatch: aws.String("*")})
	if e == nil {
		return nil
	}
	var api smithy.APIError
	if !errors.As(e, &api) || (api.ErrorCode() != "PreconditionFailed" && api.ErrorCode() != "ConditionalRequestConflict") {
		return e
	}
	info, e := s.Info(ctx, key)
	if e != nil {
		return e
	}
	if info.Size != int64(len(data)) || info.Checksum != checksum {
		return core.Conflict
	}
	return nil
}
