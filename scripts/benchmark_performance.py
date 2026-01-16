#!/usr/bin/env python3
"""
Performance benchmark comparing SQLite vs Qdrant vector search.
"""

import sqlite3
import requests
import json
import time
import numpy as np
from typing import List, Dict, Any

def generate_query_vector(dim=768):
    """Generate a random query vector."""
    return np.random.normal(0, 1, dim).tolist()

def benchmark_sqlite_search(db_path: str, query_vector: List[float], limit: int = 5) -> Dict[str, Any]:
    """Benchmark SQLite search (simplified cosine similarity)."""
    start_time = time.time()

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Get all embeddings (simplified - in practice you'd use an index)
    cursor.execute("SELECT id, vector, text, path FROM embeddings")
    rows = cursor.fetchall()

    results = []
    query_vec = np.array(query_vector)

    for row in rows:
        emb_id, vector_str, text, path = row
        emb_vec = np.array(json.loads(vector_str))

        # Calculate cosine similarity
        similarity = np.dot(query_vec, emb_vec) / (np.linalg.norm(query_vec) * np.linalg.norm(emb_vec))

        results.append({
            'id': emb_id,
            'score': float(similarity),
            'text': text,
            'path': path
        })

    # Sort by similarity and take top results
    results.sort(key=lambda x: x['score'], reverse=True)
    results = results[:limit]

    end_time = time.time()

    conn.close()

    return {
        'results': results,
        'duration': end_time - start_time,
        'method': 'sqlite'
    }

def benchmark_qdrant_search(qdrant_url: str, collection: str, query_vector: List[float], limit: int = 5) -> Dict[str, Any]:
    """Benchmark Qdrant vector search."""
    start_time = time.time()

    payload = {
        'vector': query_vector,
        'limit': limit,
        'with_payload': True
    }

    response = requests.post(f"{qdrant_url}/collections/{collection}/points/search", json=payload)
    response.raise_for_status()

    data = response.json()
    results = []

    for point in data['result']:
        results.append({
            'id': str(point['id']),
            'score': point['score'],
            'text': point['payload']['text'],
            'path': point['payload']['path']
        })

    end_time = time.time()

    return {
        'results': results,
        'duration': end_time - start_time,
        'method': 'qdrant'
    }

def run_benchmark(sqlite_db: str, qdrant_url: str, collection: str, num_queries: int = 10):
    """Run performance comparison benchmark."""
    print(f"Running performance benchmark with {num_queries} queries...")
    print("=" * 60)

    sqlite_times = []
    qdrant_times = []

    for i in range(num_queries):
        query_vector = generate_query_vector()

        # Benchmark SQLite
        sqlite_result = benchmark_sqlite_search(sqlite_db, query_vector)
        sqlite_times.append(sqlite_result['duration'])

        # Benchmark Qdrant
        qdrant_result = benchmark_qdrant_search(qdrant_url, collection, query_vector)
        qdrant_times.append(qdrant_result['duration'])

        print(".4f")

        # Verify results are similar (first result should be reasonably close)
        if i == 0:  # Only check first query in detail
            sqlite_top_score = sqlite_result['results'][0]['score']
            qdrant_top_score = qdrant_result['results'][0]['score']
            print(".4f")
    print()
    print("Performance Summary:")
    print("-" * 30)
    print("SQLite  - Avg: .4f")
    print("Qdrant  - Avg: .4f")
    print(".2f")
    print()
    print("Raw Times:")
    print(f"SQLite:  {sqlite_times}")
    print(f"Qdrant: {qdrant_times}")

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Benchmark SQLite vs Qdrant performance")
    parser.add_argument("--sqlite-db", default="test_embeddings.db", help="SQLite database path")
    parser.add_argument("--qdrant-url", default="http://localhost:6333", help="Qdrant server URL")
    parser.add_argument("--collection", default="test_rag", help="Qdrant collection name")
    parser.add_argument("--num-queries", type=int, default=10, help="Number of benchmark queries")

    args = parser.parse_args()

    run_benchmark(args.sqlite_db, args.qdrant_url, args.collection, args.num_queries)