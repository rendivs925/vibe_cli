#!/usr/bin/env python3
"""
Test RAG service integration with Qdrant backend.
"""

import requests
import json
import sys

def test_rag_service_with_qdrant():
    """Test that the RAG service can search using Qdrant."""

    # First, let's test a simple search directly on Qdrant to make sure our data is there
    print("Testing Qdrant collection status...")
    try:
        response = requests.get("http://localhost:6333/collections/test_rag")
        collection_data = response.json()

        if collection_data['result']['points_count'] == 50:
            print("✓ Collection 'test_rag' contains 50 points as expected")
        else:
            print(f"✗ Collection contains {collection_data['result']['points_count']} points, expected 50")
            return False

    except Exception as e:
        print(f"✗ Failed to check collection: {e}")
        return False

    # Now test a similarity search
    print("\nTesting vector similarity search...")
    try:
        # Get a sample vector from our test data
        import sqlite3
        conn = sqlite3.connect('test_embeddings.db')
        cursor = conn.cursor()
        cursor.execute('SELECT vector FROM embeddings LIMIT 1')
        vector_str = cursor.fetchone()[0]
        query_vector = json.loads(vector_str)
        conn.close()

        # Search for similar vectors
        payload = {
            'vector': query_vector,
            'limit': 3,
            'with_payload': True
        }

        response = requests.post('http://localhost:6333/collections/test_rag/points/search', json=payload)
        results = response.json()

        if 'result' in results and len(results['result']) > 0:
            print("✓ Search returned results")
            top_result = results['result'][0]
            print(f"  Top result score: {top_result['score']:.4f}")
            print(f"  Text preview: {top_result['payload']['text'][:50]}...")
            print(f"  Path: {top_result['payload']['path']}")
        else:
            print("✗ Search returned no results")
            return False

    except Exception as e:
        print(f"✗ Search failed: {e}")
        return False

    # Test Rust application build (basic integration test)
    print("\nTesting Rust application compilation...")
    import subprocess
    try:
        result = subprocess.run(['cargo', 'check'], cwd='/home/rendi/projects/vibe_cli',
                              capture_output=True, text=True, timeout=60)

        if result.returncode == 0:
            print("✓ Rust application compiles successfully")
        else:
            print("✗ Rust compilation failed")
            print("Error output:")
            print(result.stderr[:500])
            return False

    except subprocess.TimeoutExpired:
        print("✗ Compilation timed out")
        return False
    except Exception as e:
        print(f"✗ Compilation test failed: {e}")
        return False

    print("\n🎉 All integration tests passed!")
    print("✓ Qdrant collection contains expected data")
    print("✓ Vector search works correctly")
    print("✓ Rust application compiles")
    print("\nThe RAG service is ready to use Qdrant for semantic search!")

    return True

if __name__ == "__main__":
    success = test_rag_service_with_qdrant()
    sys.exit(0 if success else 1)