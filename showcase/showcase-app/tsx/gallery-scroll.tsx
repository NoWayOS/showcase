// The Screen Compiler gallery, 9 of 11: one child seen through a view that scrolls - with a mouse's wheel,
// a drag or the arrow keys - starting with paragraphs the application sets, wrapped onto as many lines
// as they need.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, HeaderBar, Screen, ScrollView, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;
const STORY_BYTES = 2048;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  story: string;
};

export function GalleryScrollScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Scroll view
          </Text>
          <Spacer />
          <Text>
            9 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Text role="caption">{'<ScrollView><Column>...</Column></ScrollView>: scroll with the wheel, a drag or the keys'}</Text>
        <ScrollView id="lines">
          <Column gap={GAP}>
            <Text maxLength={STORY_BYTES}>{props.story}</Text>
            <Text role="caption">{'<Text maxLength={2048}>{props.story}</Text>: paragraphs the application sets'}</Text>
            <Text>Line 1: the top</Text>
            <Text>Line 2</Text>
            <Text>Line 3</Text>
            <Text>Line 4</Text>
            <Text>Line 5</Text>
            <Text>Line 6</Text>
            <Text>Line 7</Text>
            <Text>Line 8</Text>
            <Text>Line 9</Text>
            <Text>Line 10</Text>
            <Text>Line 11</Text>
            <Text>Line 12</Text>
            <Text>Line 13</Text>
            <Text>Line 14</Text>
            <Text>Line 15</Text>
            <Text>Line 16: the end</Text>
          </Column>
        </ScrollView>
      </Column>
    </Screen>
  );
}
