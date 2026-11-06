import contextlib
import io
import subprocess
import unittest
from run_runtime_self_test import run


class RetryTests(unittest.TestCase):
    def check(self, messages, expected, attempts):
        calls = []
        delays = []
        def execute(command, **kwargs):
            calls.append(command)
            code, text = messages[min(len(calls)-1, len(messages)-1)]
            return subprocess.CompletedProcess(command, code, text)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(run(['engine', '--self-test'], execute, delays.append), expected)
        self.assertEqual(len(calls), attempts)
        self.assertEqual(len(delays), attempts-1)

    def test_temporary_download_can_recover(self):
        for code in (429, 502, 503, 504):
            self.check([(1, f'error: could not download the Koett voice detector: http status: {code}\n'), (0, 'passed\n')], 0, 2)

    def test_retries_are_bounded(self):
        self.check([(1, 'error: could not download the Koett speech model: http status: 504\n')], 1, 3)

    def test_permanent_integrity_and_inference_errors_fail_immediately(self):
        for message in ('error: unexpected transcript', 'error: checksum mismatch', 'error: could not download the Koett voice detector: http status: 404'):
            self.check([(1, message)], 1, 1)

    def test_crash_is_not_retried(self):
        self.check([(-11, 'native crash')], -11, 1)

    def test_success_is_not_repeated(self):
        self.check([(0, 'passed')], 0, 1)


if __name__ == '__main__':
    unittest.main()
