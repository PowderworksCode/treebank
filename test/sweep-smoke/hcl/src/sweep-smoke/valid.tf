# A small but complete Terraform module: a block with two labels, a
# template with an interpolation, a heredoc, and a for-expression.
variable "environment" {
  type    = string
  default = "staging"
}

resource "aws_s3_bucket" "artifacts" {
  bucket = "artifacts-${var.environment}"

  tags = {
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

locals {
  bucket_arns = [for b in aws_s3_bucket.artifacts : b.arn]

  policy = <<-JSON
    {
      "Version": "2012-10-17",
      "Statement": []
    }
  JSON
}
