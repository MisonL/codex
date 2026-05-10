from __future__ import annotations

import json
from typing import Any


def response_created(response_id: str) -> dict[str, Any]:
    return {"type": "response.created", "response": {"id": response_id}}


def response_completed(response_id: str) -> dict[str, Any]:
    return {
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": 0,
                "input_tokens_details": None,
                "output_tokens": 0,
                "output_tokens_details": None,
                "total_tokens": 0,
            },
        },
    }


def function_call(call_id: str, name: str, args: dict[str, Any]) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": json.dumps(args, separators=(",", ":")),
        },
    }


def custom_tool_call(call_id: str, name: str, text: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": text,
        },
    }


def assistant_message(text: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": "msg-final",
            "content": [{"type": "output_text", "text": text}],
        },
    }


def sse(response_id: str, *events: dict[str, Any]) -> bytes:
    full = [response_created(response_id), *events, response_completed(response_id)]
    parts = []
    for event in full:
        kind = event["type"]
        parts.append(f"event: {kind}\n")
        parts.append(f"data: {json.dumps(event, separators=(',', ':'))}\n\n")
    return "".join(parts).encode()


def output_text(body: dict[str, Any], call_id: str, kind: str) -> str:
    for item in body.get("input", []):
        if item.get("type") == kind and item.get("call_id") == call_id:
            output = item.get("output")
            if isinstance(output, str):
                return output
            return json.dumps(output, sort_keys=True)
    raise AssertionError(f"missing {kind} for {call_id}")


def tool_names(body: dict[str, Any]) -> set[str]:
    return {
        tool.get("name") or tool.get("type")
        for tool in body.get("tools", [])
        if tool.get("name") or tool.get("type")
    }


def summarize_outputs(requests: list[dict[str, Any]]) -> dict[str, Any]:
    outputs: dict[str, Any] = {}
    for body in requests:
        for item in body.get("input", []):
            kind = item.get("type")
            if kind in {"function_call_output", "custom_tool_call_output"}:
                outputs[item["call_id"]] = {"type": kind, "output": item.get("output")}
    return outputs
