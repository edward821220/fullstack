IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='users' AND xtype='U')
CREATE TABLE users (
    id UNIQUEIDENTIFIER PRIMARY KEY DEFAULT NEWID(),
    email NVARCHAR(255) NOT NULL UNIQUE,
    display_name NVARCHAR(255) NOT NULL,
    role NVARCHAR(50) NOT NULL DEFAULT 'user',
    email_verified BIT NOT NULL DEFAULT 0,
    created_at DATETIME2 NOT NULL DEFAULT GETDATE(),
    updated_at DATETIME2 NOT NULL DEFAULT GETDATE(),
    version BIGINT NOT NULL DEFAULT 1
);

IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='user_identities' AND xtype='U')
CREATE TABLE user_identities (
    id UNIQUEIDENTIFIER PRIMARY KEY DEFAULT NEWID(),
    user_id UNIQUEIDENTIFIER NOT NULL FOREIGN KEY REFERENCES users(id) ON DELETE CASCADE,
    provider NVARCHAR(100) NOT NULL,
    issuer NVARCHAR(1024) NOT NULL,
    external_sub NVARCHAR(255) NOT NULL,
    created_at DATETIME2 NOT NULL DEFAULT GETDATE(),
    CONSTRAINT UQ_user_identities_provider_issuer_sub UNIQUE(provider, issuer, external_sub)
);

IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name='idx_user_identities_user_id')
CREATE INDEX idx_user_identities_user_id ON user_identities(user_id);

IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name='idx_user_identities_lookup')
CREATE INDEX idx_user_identities_lookup ON user_identities(provider, issuer, external_sub);
