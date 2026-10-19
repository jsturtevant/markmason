# Language-tagged command blocks

Put the language first and `markmason` second. Standard Markdown renderers can
highlight the code, and `mm --step` colors its source without changing commands,
assertions, or execution mode. No Python or C# runtime is needed by these examples:
they all execute the test CLI, not source code in the named language.

## Bash styling

```bash markmason
mm-test-cli echo "bash"
```

```expected
bash
```

## Python styling

```python markmason
mm-test-cli echo "python"
```

```expected
python
```

## C# styling

Use `csharp`, `c#`, or `cs`.

```csharp markmason
mm-test-cli echo "csharp"
```

```expected
csharp
```

```c# markmason
mm-test-cli echo "c#"
```

```expected
c#
```

## Unknown languages remain readable

Unsupported language tags display plain source in the walkthrough.

```custom-language markmason
mm-test-cli echo "plain source"
```

```expected
plain source
```

## Expected failures still use the same metadata

```bash markmason (exit 2)
mm-test-cli fail
```

```expected
stdout
stderr
```

## Ordinary code fences still do not execute

The `markmason` marker is required, so these are just documentation.

```python
raise RuntimeError("must not execute")
```

```csharp
throw new Exception("must not execute");
```
