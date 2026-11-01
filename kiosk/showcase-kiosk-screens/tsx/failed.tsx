// Kiosk reference screen, phase Failed: why the answers were not
// sent, how many tries are left, and a retry only when one may help - which
// the application works out (not rejected, and a try left) and passes as
// `canRetry`, as a condition is one prop in Screen Compiler's subset.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const GAP_ACTIONS = 16;
const EDGE = 32;

type FailedProps = {
  rejected: boolean;
  attemptsLeft: number;
  canRetry: boolean;
  onRetry: () => void;
  onBack: () => void;
  onCancel: () => void;
};

export function FailedScreen(props: FailedProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Your answers were not sent</Text>
        {props.rejected ? (
          <Column gap={GAP}>
            <Text>The service could not accept them.</Text>
            <Text>Check them and try again.</Text>
          </Column>
        ) : (
          <Text>The service did not answer in time.</Text>
        )}
        <Row gap={GAP}>
          <Text>Tries left:</Text>
          <Text>{props.attemptsLeft}</Text>
        </Row>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Check answers</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          {props.canRetry && (
            <Button id="retry" onPress={props.onRetry}>
              Try again
            </Button>
          )}
        </Row>
      </Column>
    </Screen>
  );
}
