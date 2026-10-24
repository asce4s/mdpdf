# Untrusted content

<script>alert('script')</script>

<img src="x" onerror="alert('event')">

<a href="javascript:alert('link')">unsafe link</a>

[Unsafe Markdown](javascript:alert%281%29)

<iframe srcdoc="<script>alert(1)</script>"></iframe>

<svg onload="alert(1)"><script>alert(1)</script></svg>

<form action="https://example.org"><input autofocus onfocus="alert(1)"></form>

<style>@import 'https://example.org/evil.css';</style>

Safe <strong>formatting</strong> remains.
