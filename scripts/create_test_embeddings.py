#!/usr/bin/env python3
"""
Create test SQLite database with sample embeddings for migration testing.
"""

import sqlite3
import json
import numpy as np
import random
import string

def generate_random_text(length=200):
    """Generate random text for testing."""
    words = ["the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
             "hello", "world", "python", "rust", "vector", "database", "search",
             "semantic", "embedding", "machine", "learning", "artificial", "intelligence"]
    return " ".join(random.choices(words, k=length//5))

def generate_random_vector(dim=768):
    """Generate random vector of specified dimension."""
    return np.random.normal(0, 1, dim).tolist()

def create_test_database(db_path, num_embeddings=100):
    """Create SQLite database with test embeddings."""

    # Connect to database (this will create it if it doesn't exist)
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Create embeddings table
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS embeddings (
            id TEXT PRIMARY KEY,
            vector TEXT NOT NULL,
            text TEXT NOT NULL,
            path TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    ''')

    # Generate and insert test embeddings
    print(f"Creating {num_embeddings} test embeddings...")

    for i in range(num_embeddings):
        # Generate unique ID
        emb_id = f"test_emb_{i:04d}_{''.join(random.choices(string.ascii_letters, k=8))}"

        # Generate random vector
        vector = generate_random_vector()

        # Generate random text
        text = generate_random_text(random.randint(50, 500))

        # Generate random path
        path = f"/test/docs/file_{i:04d}.{'md' if i % 2 == 0 else 'rs'}"

        # Insert into database
        cursor.execute(
            "INSERT INTO embeddings (id, vector, text, path) VALUES (?, ?, ?, ?)",
            (emb_id, json.dumps(vector), text, path)
        )

        if (i + 1) % 20 == 0:
            print(f"  Inserted {i + 1}/{num_embeddings} embeddings")

    # Commit and close
    conn.commit()
    conn.close()

    print(f"\nCreated test database '{db_path}' with {num_embeddings} embeddings")
    print("Vector dimension: 768")
    print("Sample paths generated:")
    print("  - /test/docs/file_0000.md")
    print("  - /test/docs/file_0001.rs")
    print("  - etc.")

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Create test SQLite database with embeddings")
    parser.add_argument("--db-path", default="test_embeddings.db", help="Path for test database")
    parser.add_argument("--num-embeddings", type=int, default=100, help="Number of test embeddings")

    args = parser.parse_args()

    try:
        create_test_database(args.db_path, args.num_embeddings)
    except Exception as e:
        print(f"Error creating test database: {e}")