CREATE TABLE IF NOT EXISTS personal_client (
    first_name TEXT,    
    last_name TEXT,
    email TEXT,
    phone_number TEXT,
    pesel VARCHAR(11) PRIMARY KEY NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS company_client (
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    email TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    krs VARCHAR(10) PRIMARY KEY NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS software (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    version TEXT NOT NULL,
    category TEXT NOT NULL,
    price NUMERIC(10, 2) NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS discount (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    discounted_products INTEGER REFERENCES software(id),
    percentage NUMERIC(7, 5) NOT NULL,
    start_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP NOT NULL,
    is_signed BOOLEAN NOT NULL DEFAULT FALSE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);

-- Create a unified contracts table
CREATE TABLE IF NOT EXISTS contract (
    id SERIAL PRIMARY KEY,
    contract_type TEXT NOT NULL CHECK (contract_type IN ('private', 'corporate')),
    personal_client_pesel VARCHAR(11) REFERENCES personal_client(pesel),
    company_client_krs VARCHAR(10) REFERENCES company_client(krs),
    product_id INTEGER REFERENCES software(id),
    price NUMERIC(10, 2) NOT NULL,
    start_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP NOT NULL,
    years_supported INTEGER NOT NULL,
    is_signed BOOLEAN NOT NULL DEFAULT FALSE,
    is_paid BOOLEAN NOT NULL DEFAULT FALSE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT check_client_type CHECK (
        (contract_type = 'private' AND personal_client_pesel IS NOT NULL AND company_client_krs IS NULL) OR
        (contract_type = 'corporate' AND personal_client_pesel IS NULL AND company_client_krs IS NOT NULL)
    )
);

-- Then payment table becomes simple
CREATE TABLE IF NOT EXISTS payment (
    id SERIAL PRIMARY KEY, 
    contract_id INTEGER REFERENCES contract(id),
    amount NUMERIC(10, 2) NOT NULL,
    payment_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);