#!/usr/bin/env python3
"""
Migration tool to move embeddings from SQLite to Qdrant.

This script reads all embeddings from a SQLite database and inserts them
into a Qdrant collection, enabling the transition to vector-based semantic search.

Usage:
    python3 scripts/migrate_to_qdrant.py --sqlite-db path/to/embeddings.db --qdrant-url http://localhost:6334 --collection vibe_rag

Requirements:
    - sqlite3
    - requests (for Qdrant HTTP API)
    - numpy (optional, for vector processing)
"""

import sqlite3
import requests
import json
import argparse
import sys
from typing import List, Dict, Any, Optional
import time

class QdrantMigrator:
    def __init__(self, qdrant_url: str, collection_name: str):
        self.qdrant_url = qdrant_url.rstrip('/')
        self.collection_name = collection_name
        self.session = requests.Session()

    def collection_exists(self) -> bool:
        """Check if collection exists in Qdrant."""
        try:
            response = self.session.get(f"{self.qdrant_url}/collections/{self.collection_name}")
            return response.status_code == 200
        except:
            return False

    def create_collection(self, vector_dim: int = 768) -> bool:
        """Create collection if it doesn't exist."""
        if self.collection_exists():
            print(f"Collection '{self.collection_name}' already exists")
            return True

        payload = {
            "vectors": {
                "size": vector_dim,
                "distance": "Cosine"
            }
        }

        try:
            response = self.session.put(
                f"{self.qdrant_url}/collections/{self.collection_name}",
                json=payload
            )
            if response.status_code in [200, 201]:
                print(f"Created collection '{self.collection_name}'")
                return True
            else:
                print(f"Failed to create collection: {response.text}")
                return False
        except Exception as e:
            print(f"Error creating collection: {e}")
            return False

    def insert_embeddings(self, embeddings: List[Dict[str, Any]], batch_size: int = 100) -> int:
        """Insert embeddings into Qdrant in batches."""
        total_inserted = 0

        for i in range(0, len(embeddings), batch_size):
            batch = embeddings[i:i + batch_size]

            points = []
            for emb in batch:
                # Convert vector string back to list if needed
                vector = emb['vector']
                if isinstance(vector, str):
                    # Assume it's stored as JSON string
                    vector = json.loads(vector)

                point = {
                    "id": hash(emb['id']) % (2**63),  # Generate numeric ID from string
                    "vector": vector,
                    "payload": {
                        "text": emb['text'],
                        "path": emb['path']
                    }
                }
                points.append(point)

            payload = {"points": points}

            try:
                response = self.session.put(
                    f"{self.qdrant_url}/collections/{self.collection_name}/points",
                    json=payload
                )

                if response.status_code in [200, 201]:
                    total_inserted += len(points)
                    print(f"Inserted batch of {len(points)} embeddings (total: {total_inserted})")
                else:
                    print(f"Failed to insert batch: {response.text}")

            except Exception as e:
                print(f"Error inserting batch: {e}")

            # Small delay to avoid overwhelming the server
            time.sleep(0.1)

        return total_inserted

def read_sqlite_embeddings(db_path: str) -> List[Dict[str, Any]]:
    """Read all embeddings from SQLite database."""
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()

        # Check if table exists
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='embeddings'")
        if not cursor.fetchone():
            print("No 'embeddings' table found in SQLite database")
            return []

        # Read all embeddings
        cursor.execute("SELECT id, vector, text, path FROM embeddings")
        rows = cursor.fetchall()

        embeddings = []
        for row in rows:
            id_str, vector_str, text, path = row

            # Parse vector (assume it's stored as JSON)
            try:
                vector = json.loads(vector_str)
            except:
                print(f"Warning: Could not parse vector for embedding {id_str}")
                continue

            embeddings.append({
                'id': id_str,
                'vector': vector,
                'text': text,
                'path': path
            })

        conn.close()
        print(f"Read {len(embeddings)} embeddings from SQLite database")
        return embeddings

    except Exception as e:
        print(f"Error reading SQLite database: {e}")
        return []

def main():
    parser = argparse.ArgumentParser(description="Migrate embeddings from SQLite to Qdrant")
    parser.add_argument("--sqlite-db", required=True, help="Path to SQLite database")
    parser.add_argument("--qdrant-url", default="http://localhost:6334", help="Qdrant server URL")
    parser.add_argument("--collection", default="vibe_rag", help="Qdrant collection name")
    parser.add_argument("--vector-dim", type=int, default=768, help="Vector dimension")
    parser.add_argument("--batch-size", type=int, default=100, help="Batch size for insertion")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be migrated without actually doing it")

    args = parser.parse_args()

    print("Starting SQLite to Qdrant migration...")
    print(f"SQLite DB: {args.sqlite_db}")
    print(f"Qdrant URL: {args.qdrant_url}")
    print(f"Collection: {args.collection}")
    print(f"Vector dimension: {args.vector_dim}")
    print(f"Batch size: {args.batch_size}")
    print(f"Dry run: {args.dry_run}")
    print()

    # Read embeddings from SQLite
    embeddings = read_sqlite_embeddings(args.sqlite_db)
    if not embeddings:
        print("No embeddings found to migrate")
        sys.exit(1)

    if args.dry_run:
        print(f"DRY RUN: Would migrate {len(embeddings)} embeddings")
        for i, emb in enumerate(embeddings[:5]):  # Show first 5
            print(f"  {i+1}: {emb['id'][:50]}... (vector dim: {len(emb['vector'])})")
        if len(embeddings) > 5:
            print(f"  ... and {len(embeddings) - 5} more")
        print("\nUse --dry-run=false to perform actual migration")
        return

    # Initialize Qdrant migrator
    migrator = QdrantMigrator(args.qdrant_url, args.collection)

    # Create collection if needed
    if not migrator.create_collection(args.vector_dim):
        print("Failed to create/access collection")
        sys.exit(1)

    # Perform migration
    print(f"Starting migration of {len(embeddings)} embeddings...")
    start_time = time.time()

    total_inserted = migrator.insert_embeddings(embeddings, args.batch_size)

    end_time = time.time()
    duration = end_time - start_time

    print("\nMigration completed!")
    print(f"Total embeddings processed: {len(embeddings)}")
    print(f"Successfully inserted: {total_inserted}")
    print(f"Duration: {duration:.2f} seconds")
    if total_inserted > 0 and duration > 0:
        print(".2f")

    if total_inserted < len(embeddings):
        print(f"Warning: {len(embeddings) - total_inserted} embeddings failed to insert")
        sys.exit(1)

if __name__ == "__main__":
    main()