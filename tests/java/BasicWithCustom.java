import static java.lang.StringTemplate.STR;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.io.IOException;
import java.util.logging.*;

Logger logger = Logger.getLogger("basic");

void main() throws IOException {
    var fh = new FileHandler("basic-class-line.log");
    fh.setFormatter(new LineNumberFormatter());
    logger.addHandler(fh);
    logger.setLevel(Level.FINE);
    logger.fine("Hello from main");
    for (int i = 0; i < 3; i++) {
        foo(i);
    }
}

void foo(int i) {
    logger.fine(STR."Hello from foo i=\{i}");
}


public class LineNumberFormatter extends Formatter {
    @Override
    public String format(LogRecord record) {
        // Get the stack trace element to retrieve the line number
        StackTraceElement[] stackTrace = Thread.currentThread().getStackTrace();
        String lineNumber = "???";
        for (StackTraceElement element : stackTrace) {
            if (element.getClassName().equals(record.getSourceClassName())) {
                lineNumber = String.valueOf(element.getLineNumber());
                break;
            }
        }

        // Format the log message
        return String.format("%1$tF %1$tT %4$s %3$s:%6$s %2$s: %5$s%n",
                record.getMillis(),
                record.getSourceMethodName(),
                record.getSourceClassName(),
                record.getLevel(),
                record.getMessage(),
                lineNumber);
    }
}