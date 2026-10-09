package adapters

import (
	"context"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/service/s3"
	"github.com/aws/aws-sdk-go-v2/service/s3/types"
	"io"
	"mime"
	"time"
)

type S3 struct {
	Client  *s3.Client
	Presign *s3.PresignClient
	Bucket  string
}

func NewS3(ctx context.Context, bucket, region string) (*S3, error) {
	cfg, e := config.LoadDefaultConfig(ctx, config.WithRegion(region))
	if e != nil {
		return nil, e
	}
	c := s3.NewFromConfig(cfg)
	return &S3{c, s3.NewPresignClient(c), bucket}, nil
}
func (s *S3) UploadURL(ctx context.Context, key, media string, size int64, checksum string) (string, error) {
	r, e := s.Presign.PresignPutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key), ContentType: aws.String(media), ContentLength: aws.Int64(size), ChecksumSHA256: aws.String(checksum)}, s3.WithPresignExpires(5*time.Minute))
	if e != nil {
		return "", e
	}
	return r.URL, nil
}
func (s *S3) Info(ctx context.Context, key string) (core.ObjectInfo, error) {
	h, e := s.Client.HeadObject(ctx, &s3.HeadObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key), ChecksumMode: types.ChecksumModeEnabled})
	if e != nil {
		return core.ObjectInfo{}, e
	}
	r, e := s.Client.GetObject(ctx, &s3.GetObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key), Range: aws.String("bytes=0-511")})
	if e != nil {
		return core.ObjectInfo{}, e
	}
	defer r.Body.Close()
	prefix, e := io.ReadAll(io.LimitReader(r.Body, 512))
	return core.ObjectInfo{Size: aws.ToInt64(h.ContentLength), Checksum: aws.ToString(h.ChecksumSHA256), Prefix: prefix}, e
}
func (s *S3) DownloadURL(ctx context.Context, key, name string) (string, error) {
	d := mime.FormatMediaType("attachment", map[string]string{"filename": name})
	r, e := s.Presign.PresignGetObject(ctx, &s3.GetObjectInput{Bucket: aws.String(s.Bucket), Key: aws.String(key), ResponseContentDisposition: aws.String(d), ResponseContentType: aws.String("application/octet-stream")}, s3.WithPresignExpires(5*time.Minute))
	if e != nil {
		return "", fmt.Errorf("s3 presign: %w", e)
	}
	return r.URL, nil
}
