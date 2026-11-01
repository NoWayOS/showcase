// Pay at the pump, approved: the card reader approved up to the
// amount, every choice already wiped from both sides, and the screen
// follows the pump - ready, fuelling, all done - as the confirmation
// counts down, then clears itself for the next customer. The pounds and
// litres count up, simulated and sped up; the pump's own display is the
// official reading; a bar fills as the amount is taken. Fictional
// brand; every text invented.
import { Button, Card, Column, ProgressBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type FuellingProps = {
  approved: string;
  stage: string;
  cost: string;
  litres: string;
  fuelled: number;
  secondsLeft: number;
  onDone: () => void;
};

export function FuellingScreen(props: FuellingProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Row gap={GAP}>
          <Text role="label">Approved up to</Text>
          <Text weight="bold">{props.approved}</Text>
        </Row>
        <Text id="title" role="title">
          {props.stage}
        </Text>
        <Card gap={GAP} padding={TIGHT}>
          <Text role="label">Cost</Text>
          <Text id="cost" role="display">
            {props.cost}
          </Text>
          <Row gap={GAP}>
            <Text role="label">Litres</Text>
            <Text weight="bold">{props.litres}</Text>
          </Row>
          <ProgressBar value={props.fuelled} />
          <Text role="caption">Simulated and sped up. The pump's own display is the official reading.</Text>
        </Card>
        <Spacer />
        <Row gap={GAP}>
          <Text>Seconds until it clears for the next customer:</Text>
          <Text>{props.secondsLeft}</Text>
          <Button id="done" onPress={props.onDone}>
            Done
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
