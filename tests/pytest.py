
#!/usr/bin/env python3
"""
CodeLab Stress Test
Tests Python syntax-aware code organization.
"""

import asyncio
import json
import os
from pathlib import Path
from dataclasses import dataclass, field
from typing import Any, Optional, Callable

VERSION = "1.0.0"
DEBUG = True


def debug_only(func: Callable) -> Callable:
    """Simple decorator."""

    def wrapper(*args, **kwargs):
        if DEBUG:
            print(f"[DEBUG] Calling {func.__name__}")
        return func(*args, **kwargs)

    return wrapper


@dataclass
class User:
    username: str
    age: int
    permissions: list[str] = field(default_factory=list)

    def has_permission(self, permission: str) -> bool:
        return permission in self.permissions


class Database:
    def __init__(self, location: str):
        self.location = Path(location)
        self.connected = False

    def connect(self) -> None:
        self.connected = True

    def disconnect(self) -> None:
        self.connected = False


class UserDatabase(Database):
    def __init__(self, location: str):
        super().__init__(location)
        self.users: dict[str, User] = {}

    @debug_only
    def add_user(self, user: User) -> bool:
        if user.username in self.users:
            return False

        self.users[user.username] = user
        return True

    def get_user(self, username: str) -> Optional[User]:
        return self.users.get(username)


class Application:
    def __init__(self):
        self.database = UserDatabase("./users.db")
        self.running = False

    async def initialize(self) -> None:
        await asyncio.sleep(0.1)
        self.database.connect()
        self.running = True

    async def shutdown(self) -> None:
        self.database.disconnect()
        self.running = False


@debug_only
def calculate_statistics(values: list[int]) -> dict[str, float]:
    if not values:
        return {}

    return {
        "min": min(values),
        "max": max(values),
        "average": sum(values) / len(values),
    }


def create_multiplier(factor: int):
    def multiply(value: int) -> int:
        return value * factor

    return multiply


async def fetch_data(url: str) -> dict[str, Any]:
    """Simulated asynchronous request."""
    await asyncio.sleep(0.5)
    return {"url": url, "status": 200}


def main() -> None:
    app = Application()

    user = User(
        username="developer",
        age=20,
        permissions=["read", "write", "execute"],
    )

    app.database.add_user(user)

    stats = calculate_statistics([10, 20, 30, 40, 50])
    double = create_multiplier(2)

    print("User:", app.database.get_user("developer"))
    print("Statistics:", stats)
    print("Double 21:", double(21))


if __name__ == "__main__":
    main()
